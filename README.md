# yfiles

> **A rich visual file manager on libyggterm mimicking KDE Dolphin & Windows Explorer.**
> Gives any headless server or remote terminal session a fast, visual, full-featured file manager.

[![CI](https://github.com/yggdrasilhq/yfiles/actions/workflows/ci.yml/badge.svg)](https://github.com/yggdrasilhq/yfiles/actions/workflows/ci.yml)
[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](LICENSE)
[![Docs: CC BY-SA 4.0](https://img.shields.io/badge/Docs-CC_BY--SA_4.0-lightgrey.svg)](LICENSE-CC-BY-SA-4.0)

---

## What is yfiles?

`yfiles` solves a fundamental problem: managing files on remote or headless Linux machines is traditionally limited to text-only commands or heavy remote desktop tools (VNC, RDP).

Built on **libyggterm**, `yfiles` delivers the visual power and productivity of desktop file managers like KDE Dolphin and Windows Explorer directly through your terminal connection:
- **Visual Breadcrumbs & Paths:** Clickable navigation, quick history jumping, and keyboard address bar (`Ctrl+L`).
- **Multiple Layout Modes:** Details table with sortable columns (Name, Size, Modified, Permissions), Compact list, and Icon Grid.
- **Places & Devices Sidebar:** Quick bookmarks for Home, Drives, Bookmarks, and Recent Folders.
- **Safe by Default:** Move to trash with undo capability; permanent delete requires explicit confirmation.
- **Powerful File Operations:** Bulk renaming with regex substitution, file attribute inspector, recursive disk usage breakdown, and instant terminal drop (`F4`).
- **Autonomous Agent Control:** AI agents can query directories, inspect attributes, and manipulate files with deterministic JSON commands.
- **Deep Observability:** In-process `ytrace` microsecond probes for directory indexing and file operations.

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
| `Ctrl+L` / `Alt+D` | Focus & edit breadcrumb location bar |
| `F3` | Toggle split dual-pane view |
| `F4` | Drop to terminal in current directory |
| `F5` / `Ctrl+R` | Refresh active directory listing |
| `Delete` | Move selected items to Trash |
| `Shift+Delete` | Permanently delete selected items |
| `F2` | In-place bulk or single file rename |
| `Alt+Enter` | Open Properties & Metadata inspector |

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
