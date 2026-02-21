// src/main.rs

use clap::Parser;
use std::time::Duration;

use dark_pattern_blocker::client;
use dark_pattern_blocker::config::config::Config;
use dark_pattern_blocker::daemon;
use dark_pattern_blocker::format_duration::format_duration;
use dark_pattern_blocker::parse_duration::parse_time_to_seconds;
use dark_pattern_blocker::protocols::{self, ClientMessage, DaemonMessage};
use dark_pattern_blocker::socket::Connection;

const SOCKET_PATH: &str = "/tmp/dark-pattern.sock";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Parser)]
#[clap(name = "dark-pattern", version = "0.1.0", about = "Focus session manager")]
struct Cli {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Parser)]
enum Command {
    /// Start the daemon process
    Daemon,

    /// Start a focus session (e.g. "25m", "1h30m")
    Start { duration: String },

    /// Stop the current session (blocked during active session)
    Stop,

    /// Check session status
    Status,

    /// Add a website to the block list
    Add { url: String },

    /// Remove a website from the block list
    Remove { url: String },

    /// List all blocked websites
    List,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.command {
        Command::Daemon => run_daemon().await,
        cmd => run_client(cmd).await,
    }
}

// ── Daemon ──────────────────────────────────────────────────────────────────

async fn run_daemon() {
    let config = match Config::load() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to load config: {}", e);
            std::process::exit(1);
        }
    };

    let handle = match daemon::run(config).await {
        Ok(h) => h,
        Err(e) => {
            eprintln!("Failed to start daemon: {}", e);
            std::process::exit(1);
        }
    };

    tokio::signal::ctrl_c().await.expect("failed to listen for ctrl_c");
    let _ = handle.shutdown(false).await;
}

// ── Client ──────────────────────────────────────────────────────────────────

async fn run_client(cmd: Command) {
    let mut conn = match client::connect(SOCKET_PATH, CONNECT_TIMEOUT).await {
        Ok(c) => c,
        Err(_) => {
            eprintln!("Failed to connect to daemon. Is it running?");
            std::process::exit(1);
        }
    };

    let msg = match cmd {
        Command::Start { duration } => {
            let seconds = match parse_time_to_seconds(&duration) {
                Ok(s) => s,
                Err(_) => {
                    eprintln!("Invalid duration '{}'. Use formats like 25m, 1h30m, 90s", duration);
                    std::process::exit(1);
                }
            };
            ClientMessage::Start { duration: seconds }
        }
        Command::Stop => ClientMessage::Stop,
        Command::Status => ClientMessage::GetStatus,
        Command::Add { url } => ClientMessage::AddWebsite { url },
        Command::Remove { url } => ClientMessage::RemoveWebsite { url },
        Command::List => ClientMessage::ListWebsites,
        Command::Daemon => unreachable!(),
    };

    send_and_print(&mut conn, msg).await;
}

async fn send_and_print(conn: &mut Connection, msg: ClientMessage) {
    let bytes = match protocols::encode(&msg) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("Failed to encode message: {}", e);
            std::process::exit(1);
        }
    };

    if let Err(e) = conn.send(&bytes).await {
        eprintln!("Failed to send: {}", e);
        std::process::exit(1);
    }

    let response_bytes = match conn.recv().await {
        Ok(Some(b)) => b,
        Ok(None) => {
            eprintln!("Daemon closed connection");
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("Failed to receive: {}", e);
            std::process::exit(1);
        }
    };

    let response: DaemonMessage = match protocols::decode(&response_bytes) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Failed to decode response: {}", e);
            std::process::exit(1);
        }
    };

    match response {
        DaemonMessage::Started { duration } => {
            println!("Session started: {}", format_duration(duration));
        }
        DaemonMessage::Stopped => {
            println!("Session stopped");
        }
        DaemonMessage::StatusIdle => {
            println!("Idle");
        }
        DaemonMessage::StatusWithTime { time_left } => {
            println!("Active: {} remaining", format_duration(time_left));
        }
        DaemonMessage::Pong => {
            println!("Pong");
        }
        DaemonMessage::WebsiteAdded { url } => {
            println!("Added: {}", url);
        }
        DaemonMessage::WebsiteRemoved { url } => {
            println!("Removed: {}", url);
        }
        DaemonMessage::WebsiteList { websites } => {
            if websites.is_empty() {
                println!("No blocked websites");
            } else {
                for site in &websites {
                    println!("  {}", site);
                }
            }
        }
        DaemonMessage::Error(msg) => {
            eprintln!("{}", msg);
            std::process::exit(1);
        }
    }
}
