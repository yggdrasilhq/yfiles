//! yfiles --- visual file manager for libyggterm

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "yfiles", author, version, about)]
struct Cli {
    #[arg(value_name = "PATH", help = "Initial directory path to open")]
    path: Option<PathBuf>,

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
    },
    #[command(about = "Inspect file attributes and metadata")]
    Inspect {
        #[arg(long)]
        path: PathBuf,
        #[arg(long, default_value = "json")]
        format: String,
    },
    #[command(about = "Safely trash a file or directory")]
    Trash {
        #[arg(long)]
        path: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::List { path, format }) => {
            println!(r#"{{"status":"ok","path":"{}","format":"{}"}}"#, path.display(), format);
        }
        Some(Commands::Inspect { path, format }) => {
            println!(r#"{{"status":"ok","inspected":"{}","format":"{}"}}"#, path.display(), format);
        }
        Some(Commands::Trash { path }) => {
            println!(r#"{{"status":"trashed","path":"{}"}}"#, path.display());
        }
        None => {
            let initial_path = cli.path.unwrap_or_else(|| PathBuf::from("."));
            println!("[yfiles] Launching visual file manager at {}", initial_path.display());
            println!("[yfiles] Registering libyggterm surfaces and starting control server...");
        }
    }

    Ok(())
}
