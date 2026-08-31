//! yfiles — Dolphin-style visual file manager for libyggterm (Tier A).
//!
//! Two-tier shape like yedit: `yfiles --daemon` owns the Store + control
//! endpoint; `yfiles [PATH]` is a thin client that ensures the daemon, routes
//! the path into it, emits the OSC declare, and exits — the yggterm GUI keeps
//! the surface alive via control pings.

mod fs_engine;
mod manifest;
mod osc;
mod server;
mod state;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};
use std::time::Duration;
use ytrace::{Clock, Provider, Sample};

static YTRACE: once_cell::sync::Lazy<Provider> = once_cell::sync::Lazy::new(|| {
    let provider = Provider::new("yfiles", env!("CARGO_PKG_VERSION"));
    provider.register("yfiles/dir-scan", Clock::Wall, Sample::always());
    provider.register("yfiles/file-op", Clock::Wall, Sample::always());
    provider.register("yfiles/trash-op", Clock::Wall, Sample::always());
    provider.register("yfiles/render", Clock::Cpu, Sample::always());
    provider
});

#[derive(Parser, Debug)]
#[command(name = "yfiles", author, version, about = "Visual file manager for libyggterm — Dolphin/Explorer for headless and desktop hosts")]
struct Cli {
    /// Initial directory to open. None ⇒ daemon's current path.
    #[arg(value_name = "PATH", help = "Initial directory path to open")]
    path: Option<PathBuf>,

    /// Run the durable per-host daemon (normally auto-spawned).
    #[arg(long, hide = true)]
    daemon: bool,

    /// Close this terminal session's yfiles surface (daemon keeps running).
    #[arg(long)]
    close: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    #[command(about = "List directory contents headlessly as JSON")]
    List {
        #[arg(long, default_value = ".")]
        path: PathBuf,
        #[arg(long, default_value = "json")]
        format: String,
        #[arg(long, help = "Include hidden files")]
        all: bool,
        #[arg(long, default_value = "name", help = "Sort: name|size|modified|kind")]
        sort: String,
    },
    #[command(about = "Inspect file attributes and metadata")]
    Inspect {
        #[arg(long)]
        path: PathBuf,
        #[arg(long, default_value = "json")]
        format: String,
    },
    #[command(about = "Move a file or directory to OS Trash (~/.local/share/Trash)")]
    Trash {
        #[arg(long)]
        path: PathBuf,
    },
    #[command(about = "Create a new folder")]
    Mkdir {
        #[arg(long)]
        path: PathBuf,
    },
    #[command(about = "Rename a file or folder (single entry)")]
    Rename {
        #[arg(long, help = "Existing path")]
        path: PathBuf,
        #[arg(long, help = "New name (not path)")]
        new_name: String,
    },
    #[command(about = "Move files to a destination directory")]
    Move {
        #[arg(long, help = "Comma-separated source paths")]
        sources: String,
        #[arg(long)]
        destination: PathBuf,
    },
}

fn state_dir() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    state::Store::state_dir(&home)
}

fn control_url_path() -> PathBuf {
    state_dir().join("control-url")
}

fn ping(control_url: &str) -> Option<String> {
    use std::io::{Read as _, Write as _};
    let address = control_url.strip_prefix("http://")?;
    let (host_port, _) = address.split_once('/').unwrap_or((address, ""));
    let mut stream = std::net::TcpStream::connect_timeout(
        &host_port.parse().ok()?,
        Duration::from_millis(800),
    )
    .ok()?;
    let _ = stream.set_read_timeout(Some(Duration::from_millis(800)));
    stream
        .write_all(
            format!("GET /ping HTTP/1.1\r\nHost: {host_port}\r\nConnection: close\r\n\r\n").as_bytes(),
        )
        .ok()?;
    let mut raw = String::new();
    stream.read_to_string(&mut raw).ok()?;
    let body = raw.split("\r\n\r\n").nth(1)?;
    let value: serde_json::Value = serde_json::from_str(body.trim()).ok()?;
    value.get("document_version").and_then(|v| v.as_str()).map(str::to_string)
}

fn post(control_url: &str, path: &str, body: &serde_json::Value) -> Result<serde_json::Value> {
    use std::io::{Read as _, Write as _};
    let address = control_url
        .strip_prefix("http://")
        .context("control url is not http")?;
    let (host_port, _) = address.split_once('/').unwrap_or((address, ""));
    let mut stream = std::net::TcpStream::connect_timeout(
        &host_port.parse().context("control url host:port")?,
        Duration::from_millis(1500),
    )
    .context("connecting to yfiles daemon")?;
    let _ = stream.set_read_timeout(Some(Duration::from_millis(3000)));
    let payload = body.to_string();
    stream.write_all(
        format!(
            "POST {path} HTTP/1.1\r\nHost: {host_port}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
            payload.len()
        )
        .as_bytes(),
    )?;
    let mut raw = String::new();
    stream.read_to_string(&mut raw)?;
    let body = raw.split("\r\n\r\n").nth(1).context("daemon reply has no body")?;
    serde_json::from_str(body.trim()).context("daemon reply is not json")
}

fn ensure_daemon() -> Result<String> {
    if let Ok(url) = std::fs::read_to_string(control_url_path()) {
        let url = url.trim().to_string();
        if !url.is_empty() && ping(&url).is_some() {
            return Ok(url);
        }
    }
    let exe = std::env::current_exe().context("locating yfiles binary")?;
    let mut command = std::process::Command::new(exe);
    command
        .arg("--daemon")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .current_dir(dirs::home_dir().unwrap_or_else(|| PathBuf::from("/")));
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        unsafe {
            command.pre_exec(|| {
                libc::setsid();
                Ok(())
            });
        }
    }
    command.spawn().context("spawning yfiles daemon")?;
    for _ in 0..50 {
        std::thread::sleep(Duration::from_millis(100));
        if let Ok(url) = std::fs::read_to_string(control_url_path()) {
            let url = url.trim().to_string();
            if !url.is_empty() && ping(&url).is_some() {
                return Ok(url);
            }
        }
    }
    anyhow::bail!("yfiles daemon did not come up within 5s")
}

fn run_daemon() -> Result<()> {
    manifest::write_best_effort();
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let store = state::Store::new(home);
    let server = server::spawn(store)?;
    std::fs::create_dir_all(state_dir())?;
    std::fs::write(control_url_path(), &server.url)?;
    let state = server.state.clone();
    ctrlc::set_handler(move || {
        state.lock().unwrap().store.persist_session();
        std::process::exit(0);
    })?;
    eprintln!("yfiles daemon: control endpoint at {}", server.url);
    loop {
        std::thread::park();
    }
}

fn run_headless_list(path: PathBuf, all: bool, sort: String) -> Result<()> {
    let _trace = YTRACE.span("yfiles", "dir-scan", "list", serde_json::json!({ "path": path.to_string_lossy() }));
    let sort_kind = fs_engine::SortKind::parse(&sort).unwrap_or(fs_engine::SortKind::Name);
    let entries = fs_engine::scan_directory(&path, all, sort_kind)
        .with_context(|| format!("listing {}", path.display()))?;
    let free = fs_engine::free_space(&path);
    let output = serde_json::json!({
        "ok": true,
        "path": path.to_string_lossy(),
        "count": entries.len(),
        "free_bytes": free,
        "entries": entries,
    });
    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}

fn run_headless_inspect(path: PathBuf) -> Result<()> {
    let info = fs_engine::inspect_path(&path)?;
    let output = serde_json::json!({ "ok": true, "inspect": info });
    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}

fn run_headless_trash(path: PathBuf) -> Result<()> {
    let _trace = YTRACE.span("yfiles", "trash-op", "trash", serde_json::json!({ "path": path.to_string_lossy() }));
    trash::delete(&path).with_context(|| format!("trashing {}", path.display()))?;
    println!("{}", serde_json::json!({ "ok": true, "trashed": path.to_string_lossy() }));
    Ok(())
}

fn run_headless_mkdir(path: PathBuf) -> Result<()> {
    let _trace = YTRACE.span("yfiles", "file-op", "mkdir", serde_json::json!({ "path": path.to_string_lossy() }));
    std::fs::create_dir_all(&path).with_context(|| format!("mkdir {}", path.display()))?;
    println!("{}", serde_json::json!({ "ok": true, "created": path.to_string_lossy() }));
    Ok(())
}

fn run_headless_rename(path: PathBuf, new_name: String) -> Result<()> {
    let _trace = YTRACE.span("yfiles", "file-op", "rename", serde_json::json!({ "path": path.to_string_lossy(), "new_name": new_name }));
    if new_name.contains('/') || new_name.contains('\0') || new_name.trim().is_empty() {
        anyhow::bail!("invalid new_name: {}", new_name);
    }
    let dest = path.parent().unwrap_or(Path::new(".")).join(&new_name);
    if dest.exists() {
        anyhow::bail!("destination exists: {}", dest.display());
    }
    std::fs::rename(&path, &dest).with_context(|| format!("rename {} → {}", path.display(), dest.display()))?;
    println!("{}", serde_json::json!({ "ok": true, "from": path.to_string_lossy(), "to": dest.to_string_lossy() }));
    Ok(())
}

fn run_headless_move(sources: String, destination: PathBuf) -> Result<()> {
    let _trace = YTRACE.span("yfiles", "file-op", "move", serde_json::json!({ "destination": destination.to_string_lossy() }));
    if !destination.is_dir() {
        anyhow::bail!("destination is not a directory: {}", destination.display());
    }
    let mut moved = Vec::new();
    for src_str in sources.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
        let src = PathBuf::from(src_str);
        let file_name = src.file_name().ok_or_else(|| anyhow::anyhow!("source has no file name: {}", src.display()))?;
        let dest = destination.join(file_name);
        if dest.exists() {
            anyhow::bail!("destination exists: {}", dest.display());
        }
        std::fs::rename(&src, &dest).with_context(|| format!("move {} → {}", src.display(), dest.display()))?;
        moved.push(serde_json::json!({ "from": src.to_string_lossy(), "to": dest.to_string_lossy() }));
    }
    println!("{}", serde_json::to_string_pretty(&serde_json::json!({ "ok": true, "moved": moved }))?);
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // ytrace is always available; provider is Lazy
    let _ = &*YTRACE;

    if cli.daemon {
        return run_daemon();
    }

    // Headless verbs — no daemon, deterministic JSON via fs_engine
    if let Some(cmd) = cli.command {
        let result = match cmd {
            Commands::List { path, format: _, all, sort } => run_headless_list(path, all, sort),
            Commands::Inspect { path, format: _ } => run_headless_inspect(path),
            Commands::Trash { path } => run_headless_trash(path),
            Commands::Mkdir { path } => run_headless_mkdir(path),
            Commands::Rename { path, new_name } => run_headless_rename(path, new_name),
            Commands::Move { sources, destination } => run_headless_move(sources, destination),
        };
        if let Err(e) = result {
            eprintln!("{}", serde_json::json!({ "ok": false, "error": e.to_string() }));
            std::process::exit(1);
        }
        return Ok(());
    }

    // Daemon-backed GUI mode
    let session = std::env::var("YGGTERM_SESSION_ID")
        .or_else(|_| std::env::var("LC_YGGTERM_SESSION_ID"))
        .unwrap_or_default();

    if cli.close {
        if session.is_empty() {
            anyhow::bail!("yfiles --close needs a yggterm session (YGGTERM_SESSION_ID unset)");
        }
        osc::emit_close(&session);
        println!("yfiles: surface closed (daemon keeps running).");
        return Ok(());
    }

    let control_url = ensure_daemon()?;

    if let Some(initial) = cli.path {
        let absolute = if initial.is_absolute() {
            initial
        } else {
            std::env::current_dir().map(|cwd| cwd.join(&initial)).unwrap_or(initial)
        };
        let reply = post(&control_url, "/open", &serde_json::json!({ "path": absolute.to_string_lossy() }))?;
        if reply["ok"].as_bool() != Some(true) {
            eprintln!("yfiles: open {} failed: {}", absolute.display(), reply["error"].as_str().unwrap_or("unknown error"));
        }
    }

    let version = ping(&control_url).context("yfiles daemon stopped answering")?;
    if session.is_empty() {
        eprintln!("yfiles: not inside yggterm (YGGTERM_SESSION_ID unset). Daemon at {control_url} — GUI surface needs yggterm.");
        return Ok(());
    }
    osc::emit_declare(&session, &control_url, &version);
    println!("yfiles: file manager opened — `yfiles --close` to close.");
    Ok(())
}
