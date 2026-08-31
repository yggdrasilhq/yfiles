# AGENTS.md — yfiles Engineering Contract

`yfiles` is a **libyggterm visual file manager** that brings the rich, responsive ergonomics of **KDE Dolphin and Windows Explorer** to any local or remote machine.

Running as a libyggterm application means any headless computer or remote SSH server gains an instant, lightweight, visual file manager without running an X server, Wayland compositor, or heavyweight remote desktop server.

**Repository licence: GPL-3.0-or-later (code), CC-BY-SA-4.0 (documentation).**
Do not introduce conflicting licence statements. All files under `docs/` follow Creative Commons Attribution-ShareAlike 4.0 International.

---

## 1. Core Architecture & Product Invariants

1. **Tier A libyggterm Surface Architecture:**
   - Follows `libyggterm-surfaces/SKILL.md` (OSC 7717 + loopback HTTP control server).
   - Emits pure declarative schemas:
     - **Main Viewport:** Document surface (`"placement": "viewport"`) rendering the active folder's breadcrumb bar, toolbar, file grid/list rows, and status footer.
     - **Sidebar Panel:** Contributed right-hand rail (`AppPaneRailBody`) hosting Places (Home, Root, Mounts, Bookmarks), Quick Access, and File Details/Preview.
2. **KDE Dolphin / Windows Explorer Ergonomics:**
   - **Breadcrumb Navigation:** Clickable path segments with instant keyboard navigation (`Ctrl+L` / `Alt+D` to edit path).
   - **Flexible Views:** Detailed list view (Name, Size, Modified, Mode), Compact list view, and Icon/Grid view.
   - **Split / Dual-Pane Capability:** Fast side-by-side directory comparisons and transfers (`F3` split toggle).
   - **Places & Devices:** Quick bookmarks for standard user directories (`~/Downloads`, `~/Documents`, `~/projects`), mounted drives, and removable storage.
   - **Status Footer:** Live display of item counts, selected items, total selected size, and filesystem free space.
3. **Safe & Deterministic File Operations:**
   - Default deletion moves files to OS Trash (`gio trash` / `~/.local/share/Trash`) with an explicit undo stack.
   - Permanent deletion requires deliberate `--permanent` / `Shift+Delete` flags with confirmation.
   - Bulk rename with pattern matching, replacement previews, and sequence counters.
   - In-place terminal spawn: drop directly to an interactive shell or agent session in the current folder (`F4`).
4. **Deep Observability with ytrace:**
   - Built-in `ytrace` probes record directory scan latencies, metadata extraction times, thumbnail generation intervals, and schema rendering cycles.
5. **Headless Agent Orchestration:**
   - First-class agent CLI verbs allow AI agents to navigate directories, inspect metadata, batch select, and execute file operations deterministically with structured JSON outputs.

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
