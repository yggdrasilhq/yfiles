# AGENTS.md — yfiles Engineering Contract

`yfiles` is a **libyggterm visual file manager** that brings the rich, responsive ergonomics of **KDE Dolphin and Windows Explorer** to any local or remote machine.

Running as a libyggterm application means any headless computer or remote SSH server gains an instant, lightweight, visual file manager without running an X server, Wayland compositor, or heavyweight remote desktop server.

**Repository licence: GPL-3.0-or-later (code), CC-BY-SA-4.0 (documentation).**
Do not introduce conflicting licence statements. All files under `docs/` follow Creative Commons Attribution-ShareAlike 4.0 International.

---

## 1. Core Architecture & Product Invariants

1. **Tier A libyggterm Site (browser = yggterm, glue = libyggterm, site = yfiles):**
   - Follows `libyggterm-surfaces/SKILL.md` (OSC 7717 `sidebar ; declare` + loopback `GET /pane/{doc,places,preview}` + `POST /action`, `GET /ping` liveness + `document_version` stamp). Host-resident `yfiles --daemon` at `~/.yggterm/yfiles/` (`control-url`, `session.json`); thin client `yfiles [PATH]` ensures daemon, `POST /open` with client-cwd-resolved path, emits declare, heartbeat `~4s`.
   - Declares **viewport `doc`** (browse list OR Picasa image carousel) + **rail `places`** + **rail `preview`** — `AppPaneRailBody` renders with generic widgets (`section`, `toolbar`, `list-row`, `search-box`, `tabs`, `label`, `markdown`, `text-input`, `footer`). No `F3` split, no `F4` terminal — browser already ships splits/terminals; two folders = two `yfiles` rows split by `yggterm`.
2. **Dolphin/Explorer Core (basic file manager only):**
   - **Breadcrumbs:** Clickable segments (`/`, `home`, `user`, …) + `↑` parent + `Ctrl+L` address bar with `~` expansion.
   - **Browse:** dirs-first `list-row` details (Name, Size, Modified, Kind/mime via `mime_guess` + `humansize`), `search-box` filter, `tabs` sort (name/size/modified/kind), `Show hidden` toggle.
   - **Picasa viewer:** Image open (`jpg/jpeg/png/gif/webp/bmp/svg`, `image/*`) → viewport `markdown` `![…](file://)` large preview + `‹`/`›` + filmstrip `list-row`s; preview pane mirrors.
   - **Places & Devices:** `section "Places"` (Home, Root, Documents/Downloads/Pictures/Videos when present) + `section "Devices"` (mounts from `/proc/mounts`) + `section "Tabs"` (open folders, `+`/`✕`) + `Recent`.
   - **Preview & Properties:** `section card:true "Preview"` (image thumbnail or type label) + inspect labels (Name/Path/Type/Size/Modified/Mode) + `Actions` toolbar (Rename/Trash). Footers: viewport (count + free via `statvfs`), preview (full path).
3. **Safe & Deterministic File Operations (OS Trash, not a private stack):**
   - `Delete`/`🗑` → `trash` crate → freedesktop Trash (`~/.local/share/Trash/files`+`info` or `.Trash-$UID` per filesystem; `gio trash --list` to see, file manager to restore). No `Shift+Delete` permanent, no private undo stack.
   - `＋ New folder` → `text-input` → `mkdir -p`; `F2` / right-click `Rename` → in-place `list-row rename` (`rename:<path>` value, `Enter` apply / `Esc` cancel, slash-rejecting sanitizer).
   - Non-image files delegate to `yedit`/`xdg-open` via selection; headless `move` covers batch.
4. **Deep Observability with ytrace:**
   - Wall probes `yfiles/dir-scan`, `file-op`, `trash-op` + cpu `yfiles/render`; headless verbs and `POST /action` share the same `fs_engine` so headed/headless cannot diverge.
5. **Headless Agent Orchestration (deterministic JSON):**
   - Same `fs_engine` scan/inspect/trash/mkdir/rename/move as the GUI; every verb prints pretty JSON `{ok, …}` — the agent truth and the GUI truth are one function.

---

## 2. Headless Verbs for Agents

Agents can drive `yfiles` programmatically:

```bash
# Query active directory state
yfiles list --path /home/user/workspace --format json

# Inspect file metadata
yfiles inspect --path /home/user/workspace/report.pdf --format json

# Perform safe trash
yfiles trash --path /home/user/workspace/temp.log

# Perform batch move
yfiles move --sources "a.txt,b.txt" --destination /home/user/archive/
```

---

## 3. Privacy & Public Repository Directives

- **Invent Every Example:** Use `/home/user/documents/sample.pdf`, `widgets/project`, `example.org`.
- **Zero Personal Data:** Never hardcode live user names, real file system structures, private graph paths, or credentials.
- **Scratch Space:** Disk-backed `~/.yggterm/scratchpad/` on fleet hosts — never `/tmp` (tmpfs/RAM).

---

## 4. Verification & Testing

- `cargo test` must pass cleanly before every commit.
- Verify schema generation and control endpoint responses (`/ping`, `/pane/places`, `/action`) independently of pixel rendering.
- All commits signed off with `git commit -s`.
