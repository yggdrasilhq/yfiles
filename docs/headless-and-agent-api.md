# Headless & Agent Automation API

`yfiles` is headless-friendly — every GUI view reads through `fs_engine`, and the
same engine backs deterministic JSON CLI verbs for agents. No GUI needed.

## 1. JSON CLI Verbs

All verbs exit 0 on success and print pretty JSON to stdout; on error they print `{"ok":false,"error":"…"}` and exit 1.

```bash
# List directory contents with full metadata (dirs first, then files)
yfiles list --path /home/user/workspace --format json
yfiles list --path /home/user/workspace --all --sort name   # --all shows dotfiles; --sort name|size|modified|kind

# Inspect file attributes (size, mime, mode, hidden, child_count for dirs)
yfiles inspect --path /home/user/workspace/Cargo.toml --format json

# Move to OS Trash (freedesktop spec: ~/.local/share/Trash or per-filesystem .Trash-$UID)
yfiles trash --path /home/user/workspace/temp_build.log

# Create directory (parents as needed)
yfiles mkdir --path /home/user/workspace/new_folder

# Rename a single entry (new_name is a name, not a path)
yfiles rename --path /home/user/workspace/old.txt --new-name new.txt

# Move one or more entries into a destination directory
yfiles move --sources "/home/user/workspace/a.txt,/home/user/workspace/b.txt" --destination /home/user/archive
```

Output shapes:

* `list`: `{ok, path, count, free_bytes, entries: [FileEntry]}` where `FileEntry = {name, path, is_dir, is_hidden, size, size_display, mime, extension, mode, modified, modified_display, icon}`
* `inspect`: `{ok, inspect: InspectInfo}` with `InspectInfo = {path, name, is_dir, is_hidden, size, size_display, mime, extension, mode, mode_display, modified, modified_display, is_image, is_video, is_audio, child_count}`
* `trash/mkdir/rename/move`: `{ok, trashed|created|from/to|moved}`

Filter and sort are client-side: `yfiles list --all --sort size` returns the same ordering the GUI's `tabs` sort and `toggle_hidden` show — headed/headless cannot diverge because both call `scan_directory`.

## 2. Telemetry and Traceability

All operations emit `ytrace` spans: `yfiles/dir-scan`, `yfiles/file-op`, `yfiles/trash-op` (wall), plus `yfiles/render` (cpu) for schema generation. The same probes cover GUI actions (`POST /action`) and headless verbs, so a daemon wall trace and an agent JSON trace are comparable.
