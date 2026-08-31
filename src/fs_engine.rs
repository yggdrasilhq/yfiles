//! Filesystem engine for yfiles — scanning, metadata, sorting.
//!
//! Tier A site: the browser (yggterm) renders, this module owns the truth about
//! the host filesystem. Every GUI view and headless JSON verb reads through here,
//! so headed/headless cannot diverge.

use anyhow::{Context, Result};
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Sort order for directory listings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortKind {
    Name,
    Size,
    Modified,
    Kind,
}

impl SortKind {
    pub fn as_str(self) -> &'static str {
        match self {
            SortKind::Name => "name",
            SortKind::Size => "size",
            SortKind::Modified => "modified",
            SortKind::Kind => "kind",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "name" => Some(SortKind::Name),
            "size" => Some(SortKind::Size),
            "modified" => Some(SortKind::Modified),
            "kind" => Some(SortKind::Kind),
            _ => None,
        }
    }
}

/// One entry in a directory listing — the atom the GUI and JSON verbs share.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub is_hidden: bool,
    /// Bytes on disk; 0 for directories.
    pub size: u64,
    pub modified: Option<i64>,
    pub modified_display: String,
    pub mime: String,
    pub extension: String,
    /// Unix mode bits (e.g. 0o755).
    pub mode: u32,
    /// Human-readable size (e.g. "1.4 MB").
    pub size_display: String,
    /// Icon token for the widget layer: `folder`, `file:<ext>`, `image`, `video`, `audio`, etc.
    pub icon: String,
}

impl FileEntry {
    pub fn from_path(path: PathBuf, metadata: fs::Metadata) -> Self {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.to_string_lossy().into_owned());
        let is_dir = metadata.is_dir();
        let is_hidden = name.starts_with('.');
        let size = if is_dir { 0 } else { metadata.len() };
        let mode = metadata.mode() & 0o7777;
        let modified = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as i64);
        let modified_display = metadata
            .modified()
            .ok()
            .map(|t| {
                let dt: DateTime<Local> = t.into();
                dt.format("%Y-%m-%d %H:%M").to_string()
            })
            .unwrap_or_default();

        let extension = if is_dir {
            String::new()
        } else {
            path.extension()
                .map(|e| e.to_string_lossy().to_lowercase())
                .unwrap_or_default()
        };

        let mime = if is_dir {
            "inode/directory".to_string()
        } else {
            mime_guess::from_path(&path)
                .first()
                .map(|m| m.to_string())
                .unwrap_or_else(|| "application/octet-stream".to_string())
        };

        let icon = icon_for(&mime, &extension, is_dir);
        let size_display = if is_dir {
            String::new()
        } else {
            humansize::format_size(size, humansize::DECIMAL)
        };

        Self {
            name,
            path,
            is_dir,
            is_hidden,
            size,
            modified,
            modified_display,
            mime,
            extension,
            mode,
            size_display,
            icon,
        }
    }
}

fn icon_for(mime: &str, ext: &str, is_dir: bool) -> String {
    if is_dir {
        return "icon:folder".to_string();
    }
    if ext.is_empty() {
        return "file:·".to_string();
    }
    // Use file-badge for every file (badge shows extension); mime is still stored for filtering.
    // Truncate long extensions (e.g. "markdown" → "mark").
    let mut badge = ext.to_lowercase();
    badge.truncate(4);
    format!("file:{badge}")
}

/// Scan `dir`, returning entries sorted per `sort`. Missing or unreadable dir
/// yields an empty vec plus an error string for the caller to surface — never
/// panics on permission errors.
pub fn scan_directory(
    dir: &Path,
    show_hidden: bool,
    sort: SortKind,
) -> Result<Vec<FileEntry>> {
    let read_dir = fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))?;
    let mut entries = Vec::new();
    for entry in read_dir {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let path = entry.path();
        let metadata = match fs::metadata(&path) {
            Ok(m) => m,
            Err(_) => continue,
        };
        let fe = FileEntry::from_path(path, metadata);
        if !show_hidden && fe.is_hidden {
            continue;
        }
        entries.push(fe);
    }

    // Sort: dirs first, then per kind.
    entries.sort_by(|a, b| {
        if a.is_dir != b.is_dir {
            return b.is_dir.cmp(&a.is_dir);
        }
        match sort {
            SortKind::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            SortKind::Size => b.size.cmp(&a.size),
            SortKind::Modified => b.modified.cmp(&a.modified),
            SortKind::Kind => a.extension.cmp(&b.extension).then(a.name.to_lowercase().cmp(&b.name.to_lowercase())),
        }
    });

    Ok(entries)
}

/// Full metadata for a single path — the `inspect` verb and preview pane.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InspectInfo {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    pub is_hidden: bool,
    pub size: u64,
    pub size_display: String,
    pub mime: String,
    pub extension: String,
    pub mode: u32,
    pub mode_display: String,
    pub modified: Option<i64>,
    pub modified_display: String,
    pub is_image: bool,
    pub is_video: bool,
    pub is_audio: bool,
    /// For directories, number of direct children (best-effort).
    pub child_count: Option<usize>,
}

pub fn inspect_path(path: &Path) -> Result<InspectInfo> {
    let metadata = fs::metadata(path).with_context(|| format!("stat {}", path.display()))?;
    let fe = FileEntry::from_path(path.to_path_buf(), metadata);
    let child_count = if fe.is_dir {
        fs::read_dir(path).ok().map(|rd| rd.count())
    } else {
        None
    };
    let mode_display = format!("{:04o}", fe.mode);
    let is_image = fe.mime.starts_with("image/");
    let is_video = fe.mime.starts_with("video/");
    let is_audio = fe.mime.starts_with("audio/");

    Ok(InspectInfo {
        path: fe.path,
        name: fe.name,
        is_dir: fe.is_dir,
        is_hidden: fe.is_hidden,
        size: fe.size,
        size_display: fe.size_display,
        mime: fe.mime,
        extension: fe.extension,
        mode: fe.mode,
        mode_display,
        modified: fe.modified,
        modified_display: fe.modified_display,
        is_image,
        is_video,
        is_audio,
        child_count,
    })
}

/// Free space for the filesystem holding `path` (bytes). Best-effort via `statvfs`.
pub fn free_space(path: &Path) -> Option<u64> {
    #[cfg(unix)]
    {
        use std::ffi::CString;
        use std::mem::MaybeUninit;
        let c_path = CString::new(path.to_string_lossy().as_bytes()).ok()?;
        let mut stat: MaybeUninit<libc::statvfs> = MaybeUninit::uninit();
        let ret = unsafe { libc::statvfs(c_path.as_ptr(), stat.as_mut_ptr()) };
        if ret != 0 {
            return None;
        }
        let stat = unsafe { stat.assume_init() };
        Some(stat.f_bavail * stat.f_frsize as u64)
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        None
    }
}

/// Whether `path` looks like an image yfiles can preview inline (markdown `![...](file://)`).
pub fn is_image_path(path: &Path) -> bool {
    let mime_str = mime_guess::from_path(path)
        .first()
        .map(|m| m.to_string())
        .unwrap_or_default();
    if mime_str.starts_with("image/") {
        return true;
    }
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .as_deref(),
        Some("jpg") | Some("jpeg") | Some("png") | Some("gif") | Some("webp") | Some("bmp") | Some("svg")
    )
}

/// List mount points for Places (best-effort parse of `/proc/mounts`).
pub fn mount_points() -> Vec<PathBuf> {
    let mut mounts = Vec::new();
    if let Ok(content) = fs::read_to_string("/proc/mounts") {
        for line in content.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let target = parts[1];
                // Filter to real mounts, not synthetic cgroups.
                if target.starts_with('/') && !target.starts_with("/proc") && !target.starts_with("/sys") {
                    mounts.push(PathBuf::from(target));
                }
            }
        }
    }
    mounts.sort();
    mounts.dedup();
    mounts
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn scan_tmp_returns_sorted_dirs_first() {
        let tmp = std::env::temp_dir();
        let entries = scan_directory(&tmp, false, SortKind::Name).unwrap();
        // Dirs should come before files
        let first_file_idx = entries.iter().position(|e| !e.is_dir);
        let last_dir_idx = entries.iter().rposition(|e| e.is_dir);
        if let (Some(first_file), Some(last_dir)) = (first_file_idx, last_dir_idx) {
            assert!(last_dir < first_file, "dirs first in {:?}", entries.iter().map(|e| &e.name).collect::<Vec<_>>());
        }
    }

    #[test]
    fn hidden_filter_hides_dotfiles() {
        let dir = std::env::temp_dir().join("yfiles-test-hidden-filter");
        let _ = fs::create_dir_all(&dir);
        let hidden = dir.join(".hidden_probe");
        let visible = dir.join("visible_probe");
        let _ = fs::write(&hidden, b"x");
        let _ = fs::write(&visible, b"y");
        let without_hidden = scan_directory(&dir, false, SortKind::Name).unwrap();
        let with_hidden = scan_directory(&dir, true, SortKind::Name).unwrap();
        assert!(!without_hidden.iter().any(|e| e.name == ".hidden_probe"));
        assert!(with_hidden.iter().any(|e| e.name == ".hidden_probe"));
        let _ = fs::remove_file(&hidden);
        let _ = fs::remove_file(&visible);
    }

    #[test]
    fn is_image_path_recognises_jpeg_and_png() {
        assert!(is_image_path(Path::new("/home/user/photos/img.jpg")));
        assert!(is_image_path(Path::new("/home/user/photos/pic.png")));
        assert!(!is_image_path(Path::new("/home/user/docs/report.pdf")));
    }
}
