//! yfiles OSC 7717 channel — file manager's side of the libyggterm surface contract.
//!
//! yfiles is a DOCUMENT-SURFACE app (Tier A): viewport `doc` plus two rail panes
//! (`places`, `preview`). Declarations carry `document_version` so the GUI
//! refetches only when it moves.

use base64::Engine as _;
use serde_json::json;
use std::io::Write as _;

fn emit(verb: &str, action: &str, payload: &str) {
    let encoded = base64::engine::general_purpose::STANDARD.encode(payload);
    let mut stdout = std::io::stdout().lock();
    let _ = write!(stdout, "\u{1b}]7717;{verb};{action};{encoded}\u{7}");
    let _ = stdout.flush();
}

/// `sidebar ; declare` — idempotent heartbeat. `document_version` is the
/// epoch over the viewport + rail schemas.
pub fn emit_declare(session: &str, control: &str, document_version: &str) {
    let payload = json!({
        "session": session,
        "control": control,
        "app_name": "Yfiles",
        "document_version": document_version,
        "panes": [
            {
                "id": "doc",
                "icon": "🗂\u{fe0e}",
                "title": "Yfiles (Dolphin-style file manager)",
                "placement": "viewport",
            },
            {
                "id": "places",
                "icon": "🗂\u{fe0e}",
                "title": "Yfiles places & tabs",
                "placement": "rail",
            },
            {
                "id": "preview",
                "icon": "◧\u{fe0e}",
                "title": "Yfiles preview & properties",
                "placement": "rail",
            },
        ],
    });
    emit("sidebar", "declare", &payload.to_string());
}

pub fn emit_close(session: &str) {
    emit(
        "sidebar",
        "close",
        &json!({ "session": session }).to_string(),
    );
}
