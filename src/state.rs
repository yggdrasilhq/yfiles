//! yfiles store — host-resident state for the file manager.
//!
//! The browser (yggterm) renders, the site (yfiles) owns state. Like yedit's
//! `docs::Store`, but for a file manager: current path, tabs (open folders),
//! selected entry, viewer (Picasa-style carousel), sort & hidden filter.

use crate::fs_engine::{is_image_path, SortKind, scan_directory};
use anyhow::Result;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const RECENT_LIMIT: usize = 20;
const TABS_LIMIT: usize = 20;

/// Picasa-style viewer state: which image is shown and the ordered filmstrip.
#[derive(Debug, Clone)]
pub struct ViewerState {
    pub image_path: PathBuf,
    pub images: Vec<PathBuf>,
    pub index: usize,
}

/// Host-resident file manager state.
pub struct Store {
    pub current_path: PathBuf,
    pub tabs: Vec<PathBuf>,
    pub selected: Option<PathBuf>,
    pub show_hidden: bool,
    pub sort: SortKind,
    pub viewer: Option<ViewerState>,
    pub recent: Vec<PathBuf>,
    pub epoch: u64,
    home: PathBuf,
}

impl Store {
    pub fn state_dir(home: &Path) -> PathBuf {
        home.join(".yggterm").join("yfiles")
    }

    pub fn new(home: PathBuf) -> Self {
        let mut store = Self {
            current_path: dirs::home_dir().unwrap_or_else(|| PathBuf::from("/")),
            tabs: Vec::new(),
            selected: None,
            show_hidden: false,
            sort: SortKind::Name,
            viewer: None,
            recent: Vec::new(),
            epoch: 1,
            home,
        };
        store.load_session();
        // Ensure current_path exists; fall back to home.
        if !store.current_path.exists() {
            store.current_path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
        }
        if store.tabs.is_empty() {
            store.tabs.push(store.current_path.clone());
        }
        store
    }

    fn session_path(&self) -> PathBuf {
        Self::state_dir(&self.home).join("session.json")
    }

    fn load_session(&mut self) {
        let path = self.session_path();
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(_) => return,
        };
        let value: Value = match serde_json::from_slice(&bytes) {
            Ok(v) => v,
            Err(_) => return,
        };
        if let Some(p) = value["current_path"].as_str() {
            self.current_path = PathBuf::from(p);
        }
        if let Some(tabs) = value["tabs"].as_array() {
            self.tabs = tabs
                .iter()
                .filter_map(|v| v.as_str())
                .map(PathBuf::from)
                .filter(|p| p.is_absolute())
                .collect();
        }
        if let Some(show_hidden) = value["show_hidden"].as_bool() {
            self.show_hidden = show_hidden;
        }
        if let Some(sort) = value["sort"].as_str().and_then(SortKind::parse) {
            self.sort = sort;
        }
        if let Some(recent) = value["recent"].as_array() {
            self.recent = recent
                .iter()
                .filter_map(|v| v.as_str())
                .map(PathBuf::from)
                .collect();
        }
        if let Some(epoch) = value["epoch"].as_u64() {
            self.epoch = epoch.max(1);
        }
    }

    pub fn persist_session(&self) {
        let dir = Self::state_dir(&self.home);
        let _ = std::fs::create_dir_all(&dir);
        let value = json!({
            "current_path": self.current_path.to_string_lossy(),
            "tabs": self.tabs.iter().map(|p| p.to_string_lossy()).collect::<Vec<_>>(),
            "show_hidden": self.show_hidden,
            "sort": self.sort.as_str(),
            "recent": self.recent.iter().map(|p| p.to_string_lossy()).collect::<Vec<_>>(),
            "epoch": self.epoch,
        });
        let path = self.session_path();
        let tmp = dir.join("session.json.tmp");
        if let Ok(text) = serde_json::to_string_pretty(&value) {
            if std::fs::write(&tmp, text).is_ok() {
                let _ = std::fs::rename(&tmp, &path);
            }
        }
    }

    pub fn touch(&mut self) {
        self.epoch = self.epoch.wrapping_add(1);
    }

    fn push_recent(&mut self, path: PathBuf) {
        self.recent.retain(|p| p != &path);
        self.recent.insert(0, path);
        if self.recent.len() > RECENT_LIMIT {
            self.recent.truncate(RECENT_LIMIT);
        }
    }

    fn ensure_tab(&mut self, path: &Path) {
        if !self.tabs.contains(&path.to_path_buf()) {
            if self.tabs.len() >= TABS_LIMIT {
                self.tabs.remove(0);
            }
            self.tabs.push(path.to_path_buf());
        }
    }

    /// Navigate to `path` (dir). Returns Ok if dir exists and is a directory.
    pub fn navigate(&mut self, path: &Path) -> Result<()> {
        let canonical = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.current_path.join(path)
        };
        let canonical = canonical
            .canonicalize()
            .unwrap_or(canonical);
        if !canonical.exists() {
            anyhow::bail!("path does not exist: {}", canonical.display());
        }
        if !canonical.is_dir() {
            anyhow::bail!("not a directory: {}", canonical.display());
        }
        self.current_path = canonical.clone();
        self.selected = None;
        self.viewer = None;
        self.push_recent(canonical.clone());
        self.ensure_tab(&canonical);
        self.touch();
        self.persist_session();
        Ok(())
    }

    pub fn navigate_up(&mut self) -> Result<()> {
        if let Some(parent) = self.current_path.parent().map(|p| p.to_path_buf()) {
            self.navigate(&parent)
        } else {
            anyhow::bail!("already at root");
        }
    }

    pub fn switch_tab(&mut self, path: &Path) -> Result<()> {
        self.navigate(path)
    }

    pub fn close_tab(&mut self, path: &Path) {
        self.tabs.retain(|p| p != path);
        if self.tabs.is_empty() {
            self.tabs.push(self.current_path.clone());
        }
        if self.current_path == path {
            if let Some(first) = self.tabs.first().cloned() {
                let _ = self.navigate(&first);
            }
        } else {
            self.touch();
        }
        self.persist_session();
    }

    pub fn toggle_hidden(&mut self) {
        self.show_hidden = !self.show_hidden;
        self.touch();
        self.persist_session();
    }

    pub fn set_sort(&mut self, sort: SortKind) {
        self.sort = sort;
        self.touch();
        self.persist_session();
    }

    pub fn select(&mut self, path: PathBuf) {
        self.selected = Some(path);
        self.touch();
    }

    pub fn new_folder(&mut self, name: &str) -> Result<PathBuf> {
        let sanitized = sanitize_filename(name);
        if sanitized.is_empty() {
            anyhow::bail!("folder name is empty");
        }
        let new_path = self.current_path.join(&sanitized);
        if new_path.exists() {
            anyhow::bail!("already exists: {}", sanitized);
        }
        std::fs::create_dir(&new_path)?;
        self.touch();
        Ok(new_path)
    }

    pub fn rename(&mut self, from: &Path, new_name: &str) -> Result<PathBuf> {
        let sanitized = sanitize_filename(new_name);
        if sanitized.is_empty() {
            anyhow::bail!("new name is empty");
        }
        let dest = from
            .parent()
            .unwrap_or(&self.current_path)
            .join(&sanitized);
        if dest.exists() {
            anyhow::bail!("destination exists: {}", dest.display());
        }
        std::fs::rename(from, &dest)?;
        if self.selected.as_deref() == Some(from) {
            self.selected = Some(dest.clone());
        }
        if self.viewer.as_ref().map(|v| v.image_path.as_path()) == Some(from) {
            if let Some(v) = self.viewer.as_mut() {
                v.image_path = dest.clone();
                if let Some(pos) = v.images.iter().position(|p| p == from) {
                    v.images[pos] = dest.clone();
                }
            }
        }
        self.touch();
        Ok(dest)
    }

    pub fn trash(&mut self, path: &Path) -> Result<()> {
        trash::delete(path)?;
        if self.selected.as_deref() == Some(path) {
            self.selected = None;
        }
        if self.viewer.as_ref().map(|v| v.image_path.as_path()) == Some(path) {
            self.viewer = None;
        }
        self.touch();
        Ok(())
    }

    /// Open `path`: if dir → navigate, if image → viewer, else select.
    pub fn open(&mut self, path: PathBuf) -> Result<String> {
        if path.is_dir() {
            self.navigate(&path)?;
            return Ok("navigated".to_string());
        }
        if is_image_path(&path) {
            self.open_viewer(path)?;
            return Ok("viewer".to_string());
        }
        // For other files, just select; actual open delegates to yedit/xdg-open on client.
        self.select(path.clone());
        Ok("selected".to_string())
    }

    fn open_viewer(&mut self, image_path: PathBuf) -> Result<()> {
        let dir = image_path.parent().unwrap_or(&self.current_path).to_path_buf();
        let entries = scan_directory(&dir, self.show_hidden, SortKind::Name).unwrap_or_default();
        let images: Vec<PathBuf> = entries
            .into_iter()
            .filter(|e| !e.is_dir && is_image_path(&e.path))
            .map(|e| e.path)
            .collect();
        let index = images.iter().position(|p| p == &image_path).unwrap_or(0);
        self.viewer = Some(ViewerState {
            image_path,
            images,
            index,
        });
        self.touch();
        Ok(())
    }

    pub fn viewer_next(&mut self) {
        if let Some(v) = self.viewer.as_mut() {
            if v.images.is_empty() {
                return;
            }
            v.index = (v.index + 1) % v.images.len();
            v.image_path = v.images[v.index].clone();
            self.selected = Some(v.image_path.clone());
            self.touch();
        }
    }

    pub fn viewer_prev(&mut self) {
        if let Some(v) = self.viewer.as_mut() {
            if v.images.is_empty() {
                return;
            }
            if v.index == 0 {
                v.index = v.images.len() - 1;
            } else {
                v.index -= 1;
            }
            v.image_path = v.images[v.index].clone();
            self.selected = Some(v.image_path.clone());
            self.touch();
        }
    }

    pub fn close_viewer(&mut self) {
        self.viewer = None;
        self.touch();
    }

    pub fn breadcrumb_segments(&self) -> Vec<(String, PathBuf)> {
        let mut segs = Vec::new();
        let mut cur = PathBuf::new();
        for comp in self.current_path.components() {
            cur.push(comp.as_os_str());
            let label = comp.as_os_str().to_string_lossy().into_owned();
            // Skip empty root display hiccups; keep "/" as "/"
            let display = if label == "/" { "/".to_string() } else { label };
            segs.push((display, cur.clone()));
        }
        segs
    }
}

fn sanitize_filename(name: &str) -> String {
    let trimmed = name.trim();
    // Reject path traversal
    if trimmed.contains('/') || trimmed.contains('\0') {
        return String::new();
    }
    // Remove leading dot if would hide? Allow but keep.
    trimmed.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn navigate_persists_and_restores() {
        let dir = std::env::temp_dir().join("yfiles-store-test-1");
        let _ = fs::create_dir_all(&dir);
        let home = std::env::temp_dir().join("yfiles-home-1");
        let _ = fs::create_dir_all(&home);
        let mut store = Store::new(home.clone());
        let _ = store.navigate(&dir);
        assert_eq!(store.current_path.canonicalize().unwrap(), dir.canonicalize().unwrap());
        // Second store should restore
        let store2 = Store::new(home.clone());
        assert_eq!(store2.current_path.canonicalize().unwrap(), dir.canonicalize().unwrap());
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn sanitize_rejects_slash() {
        assert_eq!(sanitize_filename("a/b"), "");
        assert_eq!(sanitize_filename("  my folder  "), "my folder");
    }

    #[test]
    fn breadcrumb_segments_cover_path() {
        let home = std::env::temp_dir().join("yfiles-home-2");
        let _ = fs::create_dir_all(&home);
        let mut store = Store::new(home);
        store.current_path = PathBuf::from("/home/user/documents");
        let segs = store.breadcrumb_segments();
        assert!(segs.last().unwrap().1 == PathBuf::from("/home/user/documents"));
    }
}
