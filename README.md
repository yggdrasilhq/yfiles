# yfiles

> **A rich visual file manager on libyggterm mimicking KDE Dolphin & Windows Explorer.**
> Gives any headless server or remote terminal session a fast, visual, full-featured file manager.

[![CI](https://github.com/yggdrasilhq/yfiles/actions/workflows/ci.yml/badge.svg)](https://github.com/yggdrasilhq/yfiles/actions/workflows/ci.yml)
[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](LICENSE)
[![Docs: CC BY-SA 4.0](https://img.shields.io/badge/Docs-CC_BY--SA_4.0-lightgrey.svg)](LICENSE-CC-BY-SA-4.0)

---

## What is yfiles?

`yfiles` solves a fundamental problem: managing files on remote or headless Linux machines is traditionally limited to text-only commands or heavy remote desktop tools (VNC, RDP).

Built on **libyggterm** (Tier A site — `yggterm` is the browser, `libyggterm` the glue, `yfiles` the site), `yfiles` delivers Dolphin/Explorer ergonomics over any SSH or local terminal:
- **Visual Breadcrumbs & Paths:** Clickable segments, parent `↑`, and keyboard address bar (`Ctrl+L`) with `~` expansion.
- **Browse + Picasa Carousel:** Details list (Name, Size, Modified, Kind, mime) with dirs-first sorting; image open → full-bleed viewer via `markdown` `![…](file://)` with ←/→ stepping and filmstrip. Other files delegate to `yedit`/`xdg-open`.
- **Places & Tabs Sidebar (partitioned):** Places (Home, Root, Documents/Downloads/Pictures/Videos, Devices from `/proc/mounts`), Tabs (open folders, `+` to add, `✕` to close), Recent.
- **Preview & Properties Sidebar (partitioned):** Thumbnail (`markdown` image when image), labels (Name/Path/Type/Size/Modified/Mode), Actions (Rename/Trash). Viewer mode mirrors carousel preview.
- **Safe by Default:** `Delete` / `🗑` → OS Trash (`~/.local/share/Trash` or per-filesystem `.Trash-$UID`, restores via `gio trash --list` / file manager) — no private undo stack. `New folder` creates `mkdir -p`.
- **Autonomous Agent Control:** Deterministic JSON verbs (`list`, `inspect`, `trash`, `mkdir`, `rename`, `move`) share `fs_engine` with the GUI, so headed/headless cannot diverge.
- **Deep Observability:** In-process `ytrace` probes (`yfiles/dir-scan`, `file-op`, `trash-op`).

---

## Quick Start

### Launching yfiles
```bash
# Launch interactive visual file manager in current directory
yfiles

# Open a specific path
yfiles /var/log

# Headless directory scan for scripts or agents
yfiles list --path /home/user/workspace --format json
```

### Key Keyboard Shortcuts

| Shortcut | Action |
|---|---|
| `Ctrl+L` | Focus & edit breadcrumb location bar |
| `Enter` | Open — folder → navigate, image → Picasa viewer, other → select |
| `Backspace` / `↑` | Ascend to parent directory |
| `Ctrl+H` | Toggle hidden files (dotfiles) visibility |
| `F5` / `↻` | Refresh active directory listing |
| `F2` | In-place rename ( `rename:<path>` field, `Enter` apply / `Esc` cancel ) |
| `Delete` / `🗑` | Move selected / row item to OS Trash |
| `←` / `→` | Carousel Prev / Next (in viewer); `← Back` returns to browse |

---

## Documentation

- [Architecture Overview](docs/architecture.md)
- [Keyboard & Navigation Guide](docs/keyboard-and-gestures.md)
- [Headless & Agent Automation API](docs/headless-and-agent-api.md)
- [Agent Operating Contract](AGENTS.md)

---

## License & Legal

- **Source Code:** Licensed under the [GNU General Public License v3.0 or later (GPL-3.0-or-later)](LICENSE).
- **Documentation:** Licensed under the [Creative Commons Attribution-ShareAlike 4.0 International License (CC-BY-SA-4.0)](LICENSE-CC-BY-SA-4.0).
- **Third-Party Notices:** See [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md).
- **Trademarks:** See [TRADEMARKS.md](TRADEMARKS.md).
