// src/main.rs

use clap::Parser;
use std::time::Duration;

use dark_pattern_blocker::client;
use dark_pattern_blocker::parse_duration::parse_time_to_seconds;
use dark_pattern_blocker::format_duration::format_duration;
use dark_pattern_blocker::protocols::{self, ClientMessage, DaemonMessage};
use dark_pattern_blocker::socket::Connection;

const SOCKET_PATH: &str = "/tmp/focusd.sock"; // match your config
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Parser)]
#[clap(name = "focusd", version = "0.1.0", about = "Focus session manager")]
struct Cli {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Parser)]
enum Command {
    /// Start a focus session (e.g. "25m", "1h30m")
    Start { duration: String },

    /// Stop the current session (blocked during active session)
    Stop,

    /// Check session status
    Status,

    Ping,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let mut conn = match client::connect(SOCKET_PATH, CONNECT_TIMEOUT).await {
        Ok(c) => c,
        Err(_) => {
            eprintln!("Could not connect to daemon. Is focusd running?");
            std::process::exit(1);
        }
    };

    match cli.command {
        Command::Start { duration } => {
            let seconds = match parse_time_to_seconds(&duration) {
                Ok(s) => s,
                Err(_) => {
                    eprintln!("Invalid duration: '{}'. Use format like 25m, 1h30m, 90s", duration);
                    std::process::exit(1);
                }
            };

            let msg = ClientMessage::Start { duration: seconds };
            send_and_handle(&mut conn, msg).await;
        }

        Command::Stop => {
            send_and_handle(&mut conn, ClientMessage::Stop).await;
        }

        Command::Status => {
            send_and_handle(&mut conn, ClientMessage::GetStatus).await;
        }

        Command::Ping => {
            send_and_handle(&mut conn, ClientMessage::Ping).await;
        }
    }
}

async fn send_and_handle(conn: &mut Connection, msg: ClientMessage) {
    // Encode and send
    let bytes = protocols::encode(&msg).expect("Failed to encode message");
    if let Err(e) = conn.send(&bytes).await {
        eprintln!("Failed to send: {}", e);
        std::process::exit(1);
    }

    // Receive and decode response
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

    // Handle response
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
        DaemonMessage::Error(msg) => {
            eprintln!("{}", msg);
            std::process::exit(1);
        }
    }
}

// src/main.rs

// Dependencies: clap with derive feature
// Commands: start <duration>, stop, status, shutdown

// ── CLI Definition ──────────────────────────────────────────────
//
// #[derive(Parser)]
// - program name, version, about
//
// #[derive(Subcommand)]
// - Start { duration: String }     ← "25m", "1h30m", etc.
// - Stop
// - Status
// - Shutdown { --force flag }

// ── Main ────────────────────────────────────────────────────────
//
// 1. Parse CLI args
// 2. Connect to daemon socket using client::connect()
//    - socket path: same as config, or hardcode for now
//    - handle "connection refused" = daemon not running
// 3. Match on subcommand:
//
//    Start { duration } →
//      parse_time_to_seconds(duration)
//      send ClientMessage::Start { duration }
//      expect DaemonMessage::Started or Error
//      print confirmation + duration
//
//    Stop →
//      send ClientMessage::Stop
//      expect DaemonMessage::Stopped or Error
//      print result (will always fail during session — that's correct)
//
//    Status →
//      send ClientMessage::GetStatus
//      expect StatusIdle or StatusWithTime
//      print "Idle" or "Active: Xm Ys remaining"
//      use format_duration here
//
//    Shutdown →
//      you'll need to add ClientMessage::Shutdown to protocols.rs
//      or skip this command for now — ctrl+c / launchctl is enough
//
// 4. Handle all DaemonMessage::Error variants → print to stderr, exit 1

// ── New dependency ──────────────────────────────────────────────
// Cargo.toml: clap = { version = "4", features = ["derive"] }