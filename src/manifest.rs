//! yfiles launcher manifest — how yggterm menus learn yfiles exists.
//!
//! Written to `~/.yggterm/apps/yfiles.json` on every run. The host scans the
//! dir and deletes manifests whose binary is gone.

use anyhow::Result;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn manifest_value(binary: &Path) -> Value {
    json!({
        "name": "yfiles",
        "label": "Yfiles",
        "icon": "",
        "binary": binary.to_string_lossy(),
        "verbs": [
            { "id": "open", "label": "Open Yfiles", "args": [] },
        ],
    })
}

fn write_to(apps_dir: &Path, binary: &Path) -> Result<PathBuf> {
    std::fs::create_dir_all(apps_dir)?;
    let path = apps_dir.join("yfiles.json");
    std::fs::write(
        &path,
        serde_json::to_string_pretty(&manifest_value(binary))?,
    )?;
    Ok(path)
}

pub fn write_best_effort() {
    let Some(home) = dirs::home_dir() else { return };
    let Ok(binary) = std::env::current_exe() else { return };
    let _ = write_to(&home.join(".yggterm").join("apps"), &binary);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_names_match_the_file_stem_and_binary_is_absolute() {
        let value = manifest_value(Path::new("/usr/local/bin/yfiles"));
        assert_eq!(value["name"], "yfiles");
        assert!(value["binary"].as_str().unwrap().starts_with('/'));
        assert_eq!(value["verbs"].as_array().unwrap().len(), 1);
    }
}
