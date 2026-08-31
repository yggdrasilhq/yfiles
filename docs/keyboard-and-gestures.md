# Keyboard & Navigation Guide

`yfiles` maps familiar Dolphin/Explorer bindings to the terminal's `yggterm`
surfaces. No split-pane or embedded terminal lives in `yfiles` — two folders
side-by-side is two `yfiles` rows split by `yggterm`; a shell there is
`yggterm`'s own terminal row.

## 1. Navigation & Breadcrumbs

- `Enter`: Open selected row — folder → navigate into it, image → Picasa-style viewer (filmstrip + ←/→), other file → select + mark for `yedit`/`xdg-open` delegation.
- `Backspace` / toolbar `↑`: Ascend to parent directory.
- Breadcrumb toolbar: Click any segment (`/`, `home`, `user`, `documents`) to jump directly; `Ctrl+L` focuses the path address bar for manual entry (`~/`, `/tmp`, `/home/user/workspace`).
- `F5` / toolbar `↻`: Refresh active directory listing.
- `Ctrl+H` / toolbar `Show hidden`: Toggle dotfiles (`.` prefix) visibility.
- `Filter…` search-box: Live substring filter over `name` (case-insensitive) in the viewport.

## 2. Tabs & Places (sidebar `places` pane)

- **Places**: Home, Root, Documents/Downloads/Pictures/Videos (when present), Devices (mounts from `/proc/mounts`). Click a row → navigate.
- **Tabs**: Open folders as `list-row`s (`selected` = current). `+` adds current path as a tab. Row `✕` or right-click `Close` closes a tab. Recent paths shown below.
- Sorting: `tabs` widget (`Name` / `Size` / `Modified` / `Kind`) — dirs always first, then per-kind ordering from `fs_engine`.

## 3. Viewer (Picasa-style carousel)

When an image (`jpg/jpeg/png/gif/webp/bmp/svg`, mime `image/*`) is opened:
- Viewport shows large preview (`markdown` `![name](file://path)`) + `← Back` + `‹` / `›` toolbar + filmstrip `list-row`s for every image in the folder.
- `←` / `→` or `viewer_prev` / `viewer_next` steps the carousel; selecting a filmstrip row `viewer_jump` jumps directly; `← Back` / `Esc` / `close_viewer` returns to browse.
- Preview pane (`preview`) mirrors the viewer's current image with size/mime/mode.

## 4. File Operations

- `F2` / right-click `Rename` / preview `✎ Rename`: In-place rename — row body becomes a text field (`rename` on the row, value under `rename:<path>`). `Enter` applies, `Esc`/`✕` cancels. Names cannot contain `/` or `\0`.
- `Delete` / `🗑` action / preview `🗑 Trash`: Move to OS Trash via `trash` crate → `~/.local/share/Trash` (home filesystem) or `.Trash-$UID` on the file's filesystem (e.g. `/tmp/.Trash-$UID`). Restores via `gio trash --list` / OS file manager — no custom undo stack.
- `＋ New folder` toolbar → `text-input` → `Enter` creates `mkdir -p` under current path and selects it.
- `Move` is headless only: `yfiles move --sources "a.txt,b.txt" --destination /home/user/archive` (GUI move is via `yggterm` drag or headless verb).

## 5. Preview & Properties (sidebar `preview` pane)

- Shows `card:true` preview (image thumbnail via `markdown` if image, else type label) + labels (Name, Path, Type/mime, Size or Items, Modified, Permissions `0o755`).
- Actions toolbar: `✎ Rename` (renames selected), `🗑 Trash` (trashes selected). Viewer mode replaces properties with carousel preview.
- Footer carries full path; viewport footer carries item count and free space (`statvfs`).
