//! yfiles control endpoint: `GET /pane/doc`, `/pane/places`, `/pane/preview`,
//! plus `POST /action`. Hand-rolled HTTP over TcpListener like yedit/ychrome.

use crate::fs_engine::{free_space, inspect_path, is_image_path, mount_points, scan_directory, SortKind};
use crate::state::{Store, ViewerState};
use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// UI-local transient state alongside the durable Store.
pub struct PaneState {
    pub store: Store,
    /// Path being renamed (in-place row field).
    pub renaming: Option<PathBuf>,
    /// Whether the "new folder" input is visible.
    pub show_new_folder: bool,
    /// File list filter (search-box).
    pub filter: String,
}

pub struct Server {
    pub url: String,
    pub state: Arc<Mutex<PaneState>>,
}

pub fn spawn(store: Store) -> Result<Server> {
    let listener = TcpListener::bind("127.0.0.1:0").context("binding yfiles control server")?;
    let port = listener.local_addr()?.port();
    let state = Arc::new(Mutex::new(PaneState {
        store,
        renaming: None,
        show_new_folder: false,
        filter: String::new(),
    }));
    {
        let state = Arc::clone(&state);
        std::thread::spawn(move || {
            for incoming in listener.incoming() {
                let Ok(stream) = incoming else { continue };
                let state = Arc::clone(&state);
                std::thread::spawn(move || handle_conn(stream, &state));
            }
        });
    }
    Ok(Server {
        url: format!("http://127.0.0.1:{port}"),
        state,
    })
}

fn handle_conn(stream: TcpStream, state: &Mutex<PaneState>) {
    let Ok(peek) = stream.try_clone() else { return };
    let mut reader = BufReader::new(peek);
    let mut line = String::new();
    if reader.read_line(&mut line).is_err() {
        return;
    }
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("/");
    let (path, _query) = target.split_once('?').unwrap_or((target, ""));

    let mut content_length = 0usize;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).is_err() || header.trim().is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            if name.eq_ignore_ascii_case("content-length") {
                content_length = value.trim().parse().unwrap_or(0);
            }
        }
    }
    let body: Value = if content_length > 0 {
        let mut raw = vec![0u8; content_length];
        if reader.read_exact(&mut raw).is_err() {
            return;
        }
        serde_json::from_slice(&raw).unwrap_or(Value::Null)
    } else {
        Value::Null
    };

    match (method, path) {
        ("POST", "/open") => {
            let raw = body["path"].as_str().unwrap_or_default().to_string();
            let mut pane = state.lock().unwrap();
            if raw.is_empty() {
                respond_json(stream, 200, &json!({ "ok": true, "document_version": version_of(&pane) }));
                return;
            }
            let p = PathBuf::from(&raw);
            let result = pane.store.open(p);
            match result {
                Ok(kind) => {
                    let v = version_of(&pane);
                    respond_json(stream, 200, &json!({ "ok": true, "kind": kind, "document_version": v }));
                }
                Err(e) => {
                    respond_json(stream, 200, &json!({ "ok": false, "error": e.to_string() }));
                }
            }
        }
        ("GET", "/ping") => {
            respond_json(
                stream,
                200,
                &json!({
                    "ok": true,
                    "app_name": "Yfiles",
                    "document_version": document_version(state),
                }),
            );
        }
        ("GET", "/pane/doc") => {
            let pane = state.lock().unwrap();
            respond_json(stream, 200, &document_schema(&pane));
        }
        ("GET", "/pane/places") => {
            let pane = state.lock().unwrap();
            respond_json(stream, 200, &places_schema(&pane));
        }
        ("GET", "/pane/preview") => {
            let pane = state.lock().unwrap();
            respond_json(stream, 200, &preview_schema(&pane));
        }
        ("POST", "/action") => {
            let reply = handle_action(state, &body);
            respond_json(stream, 200, &reply);
        }
        _ => respond_json(stream, 404, &json!({})),
    }
}

fn respond_json(mut stream: TcpStream, code: u16, body: &Value) {
    let text = body.to_string();
    let header = format!(
        "HTTP/1.1 {code} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        text.len()
    );
    let _ = stream.write_all(header.as_bytes());
    let _ = stream.write_all(text.as_bytes());
}

fn version_of(pane: &PaneState) -> String {
    pane.store.epoch.to_string()
}

fn document_version(state: &Mutex<PaneState>) -> String {
    state.lock().unwrap().store.epoch.to_string()
}

/// Viewport document schema — browse grid OR Picasa-style viewer.
fn document_schema(pane: &PaneState) -> Value {
    let store = &pane.store;
    // Viewer mode takes over the viewport
    if let Some(viewer) = &store.viewer {
        return viewer_schema(viewer, pane);
    }
    browse_schema(pane)
}

fn browse_schema(pane: &PaneState) -> Value {
    let store = &pane.store;
    let mut widgets: Vec<Value> = Vec::new();

    // Breadcrumb toolbar — each segment is a button that navigates
    let segments = store.breadcrumb_segments();
    let mut crumb_buttons = Vec::new();
    for (label, path) in &segments {
        let display = if label.is_empty() { "/" } else { label };
        crumb_buttons.push(json!({
            "action": format!("navigate:{}", path.to_string_lossy()),
            "label": display,
            "title": path.to_string_lossy(),
        }));
    }
    widgets.push(json!({
        "kind": "toolbar",
        "id": "breadcrumbs",
        "buttons": crumb_buttons,
    }));

    // Actions toolbar + path input + filter
    widgets.push(json!({
        "kind": "toolbar",
        "id": "actions",
        "buttons": [
            { "action": "navigate_up", "label": "↑", "title": "Go to parent (Backspace)" },
            { "action": "toggle_new_folder", "label": "＋ New folder", "title": "Create new folder" },
            { "action": "toggle_hidden", "label": if store.show_hidden { "Hide hidden" } else { "Show hidden" }, "title": "Toggle hidden files (Ctrl+H)" },
            { "action": "refresh", "label": "↻", "title": "Refresh (F5)" },
        ],
    }));

    if pane.show_new_folder {
        widgets.push(json!({
            "kind": "text-input",
            "id": "new_folder_name",
            "placeholder": "new folder name…",
            "value": "",
            "action": "create_folder",
        }));
    }

    widgets.push(json!({
        "kind": "search-box",
        "id": "filter",
        "placeholder": "Filter…",
        "value": pane.filter,
        "action": "filter",
    }));

    // View mode + Sort tabs
    widgets.push(json!({
        "kind": "tabs",
        "id": "view_mode",
        "action": "set_view_mode",
        "active": store.view_mode.as_str(),
        "tabs": [
            { "id": "details", "label": "Details" },
            { "id": "icons", "label": "Icons" },
        ],
    }));
    widgets.push(json!({
        "kind": "tabs",
        "id": "sort",
        "action": "set_sort",
        "active": store.sort.as_str(),
        "tabs": [
            { "id": "name", "label": "Name" },
            { "id": "size", "label": "Size" },
            { "id": "modified", "label": "Modified" },
            { "id": "kind", "label": "Kind" },
        ],
    }));

    // Directory listing
    let entries = scan_directory(&store.current_path, store.show_hidden, store.sort)
        .unwrap_or_default();
    let filter = pane.filter.trim().to_lowercase();
    let mut shown = 0usize;
    for entry in &entries {
        if !filter.is_empty() && !entry.name.to_lowercase().contains(&filter) {
            continue;
        }
        shown += 1;
        let is_selected = store.selected.as_deref() == Some(entry.path.as_path());
        // Choose row action: dir navigates, file opens/selects
        let row_action = if entry.is_dir { "open_path" } else { "open_path" };
        let subtitle = if entry.is_dir {
            String::new()
        } else {
            // Dolphin details: Size  •  Date (type is already in the badge)
            if entry.size_display.is_empty() && entry.modified_display.is_empty() {
                String::new()
            } else if entry.size_display.is_empty() {
                entry.modified_display.clone()
            } else if entry.modified_display.is_empty() {
                entry.size_display.clone()
            } else {
                format!("{}  •  {}", entry.size_display, entry.modified_display)
            }
        };
        let mut row = json!({
            "kind": "list-row",
            "id": entry.path.to_string_lossy(),
            "icon": entry.icon,
            "title": entry.name,
            "subtitle": subtitle,
            "selected": is_selected,
            "row_action": row_action,
            "actions": [
                { "action": "trash", "label": "icon:trash", "title": "Move to Trash (Delete)" },
            ],
            "menu": [
                { "action": "rename", "label": "Rename", "title": "Rename (F2)" },
                { "action": "trash", "label": "Move to Trash", "title": "Move to Trash" },
                { "action": "properties", "label": "Properties", "title": "Show properties" },
            ],
        });
        // In-place rename field
        if pane.renaming.as_deref() == Some(entry.path.as_path()) {
            row["rename"] = json!({
                "value": entry.name,
                "action": "rename_apply",
                "cancel_action": "rename_cancel",
                "placeholder": "new name…",
            });
        }
        widgets.push(row);
    }

    if entries.is_empty() {
        widgets.push(json!({
            "kind": "label",
            "muted": true,
            "text": "Empty folder.",
        }));
    } else if shown == 0 {
        widgets.push(json!({
            "kind": "label",
            "muted": true,
            "text": format!("No match for \"{}\".", pane.filter),
        }));
    }

    let mut footer: Vec<Value> = Vec::new();
    let total = entries.len();
    let hidden_note = if store.show_hidden { "" } else { "" };
    let free = free_space(&store.current_path)
        .map(|b| humansize::format_size(b, humansize::DECIMAL))
        .unwrap_or_default();
    let selected_note = if let Some(sel) = &store.selected {
        format!(" Selected: {}", sel.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default())
    } else {
        String::new()
    };
    footer.push(json!({
        "kind": "label",
        "text": format!("{total} items{hidden_note}{selected_note}  •  Free: {free}"),
    }));

    json!({ "title": store.current_path.to_string_lossy(), "widgets": widgets, "footer": footer })
}

fn viewer_schema(viewer: &ViewerState, pane: &PaneState) -> Value {
    let mut widgets: Vec<Value> = Vec::new();

    let name = viewer
        .image_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let pos = format!("{} / {}", viewer.index + 1, viewer.images.len().max(1));

    widgets.push(json!({
        "kind": "toolbar",
        "id": "viewer_bar",
        "buttons": [
            { "action": "close_viewer", "label": "← Back", "title": "Back to folder" },
            { "action": "viewer_prev", "label": "‹", "title": "Previous (←)" },
            { "action": "viewer_next", "label": "›", "title": "Next (→)" },
        ],
    }));

    widgets.push(json!({
        "kind": "label",
        "text": format!("{name}  —  {pos}"),
    }));

    // Tier A image via markdown — host renders file:// images in markdown if supported;
    // otherwise the preview pane still shows the path and metadata.
    let img_src = format!("file://{}", viewer.image_path.to_string_lossy());
    widgets.push(json!({
        "kind": "markdown",
        "id": "viewer_image",
        "source": format!("![{name}]({img_src})"),
    }));

    // Filmstrip — list-row strip of all images in this folder
    if !viewer.images.is_empty() {
        widgets.push(json!({
            "kind": "section",
            "text": "Filmstrip",
        }));
        for (idx, path) in viewer.images.iter().enumerate() {
            let fname = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let selected = idx == viewer.index;
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
            let badge = if ext.is_empty() { "·".to_string() } else { ext.chars().take(4).collect::<String>() };
            widgets.push(json!({
                "kind": "list-row",
                "id": format!("strip:{}", path.to_string_lossy()),
                "icon": format!("file:{badge}"),
                "title": fname,
                "selected": selected,
                "row_action": format!("viewer_jump:{}", path.to_string_lossy()),
            }));
        }
    }

    let mut footer: Vec<Value> = Vec::new();
    footer.push(json!({
        "kind": "label",
        "text": format!("{}  {}", viewer.image_path.to_string_lossy(), pos),
    }));
    footer.push(json!({
        "kind": "button",
        "id": "viewer_close",
        "label": "Back to folder",
        "action": "close_viewer",
    }));

    // Include pane filter so rest of UI stays, but viewer has its own footer.
    let _ = pane;
    json!({ "title": name, "widgets": widgets, "footer": footer })
}

/// Sidebar pane `places`: Places + Tabs partitions.
fn places_schema(pane: &PaneState) -> Value {
    let store = &pane.store;
    let mut widgets: Vec<Value> = Vec::new();

    // Partition: Places
    widgets.push(json!({ "kind": "section", "text": "Places" }));

    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
    let places = vec![
        ("Home", home.clone()),
        ("Root", PathBuf::from("/")),
        ("Documents", home.join("Documents")),
        ("Downloads", home.join("Downloads")),
        ("Pictures", home.join("Pictures")),
        ("Videos", home.join("Videos")),
    ];
    for (label, path) in places {
        let exists = path.exists();
        let selected = store.current_path == path;
        // Only show existing places to avoid clutter; always show Home/Root
        if !exists && label != "Home" && label != "Root" {
            continue;
        }
        widgets.push(json!({
            "kind": "list-row",
            "id": format!("place:{}", path.to_string_lossy()),
            "icon": "icon:folder",
            "title": label,
            "subtitle": path.to_string_lossy(),
            "selected": selected,
            "row_action": format!("navigate:{}", path.to_string_lossy()),
        }));
    }

    // Mounts (best-effort)
    let mounts = mount_points();
    if !mounts.is_empty() {
        widgets.push(json!({ "kind": "section", "text": "Devices" }));
        for mnt in mounts.iter().take(10) {
            let label = mnt.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| mnt.to_string_lossy().into_owned());
            let selected = store.current_path == *mnt;
            widgets.push(json!({
                "kind": "list-row",
                "id": format!("mount:{}", mnt.to_string_lossy()),
                "icon": "icon:folder",
                "title": label,
                "subtitle": mnt.to_string_lossy(),
                "selected": selected,
                "row_action": format!("navigate:{}", mnt.to_string_lossy()),
            }));
        }
    }

    // Partition: Tabs (open folders)
    widgets.push(json!({
        "kind": "section",
        "text": "Tabs",
        "action": "new_tab_here",
        "action_label": "+",
        "action_title": "New tab here",
    }));
    for tab in &store.tabs {
        let name = tab
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| tab.to_string_lossy().into_owned());
        let selected = store.current_path == *tab;
        widgets.push(json!({
            "kind": "list-row",
            "id": format!("tab:{}", tab.to_string_lossy()),
            "icon": "icon:folder",
            "title": name,
            "subtitle": tab.to_string_lossy(),
            "selected": selected,
            "row_action": format!("switch_tab:{}", tab.to_string_lossy()),
            "actions": [
                { "action": format!("close_tab:{}", tab.to_string_lossy()), "label": "icon:close", "title": "Close tab" },
            ],
            "menu": [
                { "action": format!("switch_tab:{}", tab.to_string_lossy()), "label": "Open", "title": "Open this tab" },
                { "action": format!("close_tab:{}", tab.to_string_lossy()), "label": "Close", "title": "Close tab" },
            ],
        }));
    }

    // Recent
    if !store.recent.is_empty() {
        widgets.push(json!({ "kind": "section", "text": "Recent" }));
        for path in store.recent.iter().take(8) {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.to_string_lossy().into_owned());
            widgets.push(json!({
                "kind": "list-row",
                "id": format!("recent:{}", path.to_string_lossy()),
                "icon": "icon:folder",
                "title": name,
                "subtitle": path.to_string_lossy(),
                "row_action": format!("navigate:{}", path.to_string_lossy()),
            }));
        }
    }

    json!({ "title": "Places & Tabs", "widgets": widgets })
}

/// Sidebar pane `preview`: properties + image thumbnail for selected / viewer.
fn preview_schema(pane: &PaneState) -> Value {
    let store = &pane.store;
    let mut widgets: Vec<Value> = Vec::new();

    // If viewer is open, show its current image
    if let Some(viewer) = &store.viewer {
        widgets.push(json!({ "kind": "section", "text": "Preview", "card": true }));
        let img_src = format!("file://{}", viewer.image_path.to_string_lossy());
        let name = viewer
            .image_path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        widgets.push(json!({
            "kind": "markdown",
            "id": "preview_image",
            "source": format!("![{name}]({img_src})"),
        }));
        if let Ok(info) = inspect_path(&viewer.image_path) {
            widgets.push(json!({ "kind": "label", "text": format!("{}  •  {}", info.size_display, info.mime) }));
            widgets.push(json!({ "kind": "label", "text": format!("Modified: {}", info.modified_display) }));
            widgets.push(json!({ "kind": "label", "text": format!("Mode: {}", info.mode_display) }));
        }
        widgets.push(json!({
            "kind": "toolbar",
            "id": "viewer_preview_actions",
            "buttons": [
                { "action": "viewer_prev", "label": "‹ Prev" },
                { "action": "viewer_next", "label": "Next ›" },
                { "action": "close_viewer", "label": "Back" },
            ],
        }));
        return json!({ "title": "Preview", "widgets": widgets });
    }

    // Selected entry, else current folder
    let target = store
        .selected
        .as_deref()
        .unwrap_or(&store.current_path);

    widgets.push(json!({ "kind": "section", "text": "Preview", "card": true }));

    if is_image_path(target) && target.is_file() {
        let img_src = format!("file://{}", target.to_string_lossy());
        let name = target
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        widgets.push(json!({
            "kind": "markdown",
            "id": "preview_image",
            "source": format!("![{name}]({img_src})"),
        }));
    } else if target.is_dir() {
        widgets.push(json!({
            "kind": "label",
            "text": "Folder",
        }));
    } else {
        let ext = target.extension().map(|e| e.to_string_lossy().into_owned()).unwrap_or_default();
        widgets.push(json!({
            "kind": "label",
            "text": format!("File  •  {}", if ext.is_empty() { "no extension".to_string() } else { ext }),
        }));
    }

    match inspect_path(target) {
        Ok(info) => {
            widgets.push(json!({ "kind": "label", "text": format!("Name: {}", info.name) }));
            widgets.push(json!({ "kind": "label", "text": format!("Path: {}", info.path.to_string_lossy()) }));
            widgets.push(json!({ "kind": "label", "text": format!("Type: {}", info.mime) }));
            if !info.is_dir {
                widgets.push(json!({ "kind": "label", "text": format!("Size: {}", info.size_display) }));
            } else if let Some(cnt) = info.child_count {
                widgets.push(json!({ "kind": "label", "text": format!("Items: {cnt}") }));
            }
            widgets.push(json!({ "kind": "label", "text": format!("Modified: {}", info.modified_display) }));
            widgets.push(json!({ "kind": "label", "text": format!("Permissions: {}", info.mode_display) }));
            if info.is_hidden {
                widgets.push(json!({ "kind": "label", "text": "Hidden file" }));
            }
        }
        Err(e) => {
            widgets.push(json!({ "kind": "label", "muted": true, "text": format!("Cannot inspect: {e}") }));
        }
    }

    // Actions card
    widgets.push(json!({ "kind": "section", "text": "Actions", "card": true }));
    widgets.push(json!({
        "kind": "toolbar",
        "id": "preview_actions",
        "buttons": [
            { "action": "start_rename", "label": "✎ Rename", "title": "Rename (F2)" },
            { "action": "trash_selected", "label": "🗑 Trash", "title": "Move to Trash" },
        ],
    }));

    let mut footer: Vec<Value> = Vec::new();
    footer.push(json!({
        "kind": "label",
        "text": format!("{}", target.to_string_lossy()),
    }));

    json!({ "title": "Preview & Properties", "widgets": widgets, "footer": footer })
}

fn handle_action(state: &Mutex<PaneState>, body: &Value) -> Value {
    let action_raw = body["action"].as_str().unwrap_or_default().to_string();
    let values = &body["values"];
    // Split action like "navigate:/home/user" → ("navigate", "/home/user")
    let (action, arg) = match action_raw.split_once(':') {
        Some((a, b)) => (a.to_string(), Some(b.to_string())),
        None => (action_raw.clone(), None),
    };
    let mut pane = state.lock().unwrap();
    let mut toast: Option<String> = None;

    // Helper: apply filter draft if present before any action
    if let Some(filter) = values["filter"].as_str() {
        if pane.filter != filter {
            pane.filter = filter.to_string();
            pane.store.touch();
        }
    }

    match action.as_str() {
        "navigate" => {
            // From text-input path_input or breadcrumb
            let path_str = values["path_input"]
                .as_str()
                .or(arg.as_deref())
                .or(values["value"].as_str())
                .unwrap_or("")
                .to_string();
            if !path_str.is_empty() {
                let expanded = shellexpand_home(&path_str);
                if let Err(e) = pane.store.navigate(Path::new(&expanded)) {
                    toast = Some(e.to_string());
                }
            }
        }
        "navigate_up" => {
            if let Err(e) = pane.store.navigate_up() {
                toast = Some(e.to_string());
            }
        }
        "refresh" => {
            pane.store.touch();
        }
        "toggle_hidden" => {
            pane.store.toggle_hidden();
        }
        "toggle_new_folder" => {
            pane.show_new_folder = !pane.show_new_folder;
            pane.store.touch();
        }
        "create_folder" => {
            // Value under new_folder_name
            let name = values["new_folder_name"]
                .as_str()
                .or(values["value"].as_str())
                .unwrap_or("")
                .to_string();
            if name.trim().is_empty() {
                toast = Some("folder name is empty".to_string());
            } else {
                match pane.store.new_folder(&name) {
                    Ok(p) => {
                        pane.show_new_folder = false;
                        pane.store.select(p);
                    }
                    Err(e) => toast = Some(e.to_string()),
                }
            }
        }
        "filter" => {
            // Already handled above via values["filter"]
        }
        "set_sort" => {
            let sort_str = values["sort"]
                .as_str()
                .or(values["value"].as_str())
                .or(arg.as_deref())
                .unwrap_or("name");
            if let Some(sort) = SortKind::parse(sort_str) {
                pane.store.set_sort(sort);
            }
        }
        "set_view_mode" => {
            let vm_str = values["view_mode"]
                .as_str()
                .or(values["value"].as_str())
                .or(arg.as_deref())
                .unwrap_or("details");
            if let Some(vm) = crate::state::ViewMode::parse(vm_str) {
                pane.store.set_view_mode(vm);
            }
        }
        "open_path" => {
            let path_str = values["value"].as_str().or(arg.as_deref()).unwrap_or("").to_string();
            if !path_str.is_empty() {
                let p = PathBuf::from(&path_str);
                // row id is full path, so this is the path
                match pane.store.open(p.clone()) {
                    Ok(_) => {}
                    Err(e) => toast = Some(e.to_string()),
                }
            }
        }
        "switch_tab" => {
            let path_str = arg.as_deref().or(values["value"].as_str()).unwrap_or("");
            if !path_str.is_empty() {
                let p = PathBuf::from(path_str);
                if let Err(e) = pane.store.switch_tab(&p) {
                    toast = Some(e.to_string());
                }
            }
        }
        "close_tab" => {
            let path_str = arg.as_deref().or(values["value"].as_str()).unwrap_or("");
            if !path_str.is_empty() {
                pane.store.close_tab(Path::new(path_str));
            }
        }
        "new_tab_here" => {
            pane.store.ensure_tab_clone();
            pane.store.touch();
        }
        "rename" => {
            let path_str = values["value"].as_str().or(arg.as_deref()).unwrap_or("").to_string();
            if !path_str.is_empty() {
                pane.renaming = Some(PathBuf::from(path_str));
                pane.store.touch();
            }
        }
        "start_rename" => {
            if let Some(sel) = pane.store.selected.clone() {
                pane.renaming = Some(sel);
                pane.store.touch();
            } else {
                toast = Some("nothing selected to rename".to_string());
            }
        }
        "rename_apply" => {
            if let Some(target) = pane.renaming.clone() {
                // Find the rename value: key is rename:<id> where id is path string
                let key = format!("rename:{}", target.to_string_lossy());
                let new_name = values[&key]
                    .as_str()
                    .or(values["value"].as_str())
                    .unwrap_or("")
                    .to_string();
                if new_name.trim().is_empty() {
                    toast = Some("name is empty".to_string());
                } else {
                    match pane.store.rename(&target, &new_name) {
                        Ok(new_path) => {
                            pane.renaming = None;
                            pane.store.select(new_path);
                        }
                        Err(e) => toast = Some(e.to_string()),
                    }
                }
            }
        }
        "rename_cancel" => {
            pane.renaming = None;
            pane.store.touch();
        }
        "trash" => {
            let path_str = values["value"].as_str().or(arg.as_deref()).unwrap_or("").to_string();
            if !path_str.is_empty() {
                let p = PathBuf::from(&path_str);
                if let Err(e) = pane.store.trash(&p) {
                    toast = Some(e.to_string());
                }
            }
        }
        "trash_selected" => {
            if let Some(sel) = pane.store.selected.clone() {
                if let Err(e) = pane.store.trash(&sel) {
                    toast = Some(e.to_string());
                }
            } else {
                toast = Some("nothing selected".to_string());
            }
        }
        "properties" => {
            let path_str = values["value"].as_str().or(arg.as_deref()).unwrap_or("").to_string();
            if !path_str.is_empty() {
                pane.store.select(PathBuf::from(path_str));
            }
        }
        "viewer_next" => pane.store.viewer_next(),
        "viewer_prev" => pane.store.viewer_prev(),
        "close_viewer" => pane.store.close_viewer(),
        "viewer_jump" => {
            let path_str = arg.as_deref().or(values["value"].as_str()).unwrap_or("");
            if !path_str.is_empty() {
                let p = PathBuf::from(path_str);
                let _ = pane.store.open(p);
            }
        }
        _ => {
            // Unknown action — try to interpret as navigate if it looks like a path
            if action_raw.starts_with('/') || action_raw.starts_with("~/") {
                let expanded = shellexpand_home(&action_raw);
                let _ = pane.store.navigate(Path::new(&expanded));
            }
        }
    }

    let version = version_of(&pane);
    let mut reply = json!({ "ok": true, "document_version": version });
    if let Some(t) = toast {
        reply["toast"] = json!(t);
    }
    reply
}

fn shellexpand_home(s: &str) -> String {
    if s == "~" || s.starts_with("~/") {
        if let Some(home) = dirs::home_dir() {
            return s.replacen('~', &home.to_string_lossy(), 1);
        }
    }
    s.to_string()
}

// Extra helper for places pane to clone ensure tab
trait EnsureTabClone {
    fn ensure_tab_clone(&mut self);
}
impl EnsureTabClone for Store {
    fn ensure_tab_clone(&mut self) {
        let cur = self.current_path.clone();
        if !self.tabs.contains(&cur) {
            self.tabs.push(cur);
            self.touch();
            self.persist_session();
        }
    }
}
