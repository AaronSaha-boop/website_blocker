// src/main.rs

use clap::Parser;
use std::time::Duration;

use dark_pattern_blocker::client;
use dark_pattern_blocker::config::config::Config;
use dark_pattern_blocker::daemon;
use dark_pattern_blocker::parse_duration::parse_time_to_seconds;
use dark_pattern_blocker::format_duration::format_duration;
use dark_pattern_blocker::protocols::{self, ClientMessage, DaemonMessage};
use dark_pattern_blocker::socket::Connection;

const SOCKET_PATH: &str = "/tmp/dark-pattern.sock"; // match your config
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Parser)]
#[clap(name = "dark-pattern", version = "0.1.0", about = "Focus session manager")]
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

    Daemon,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.command {
        Command::Daemon => run_daemon().await,
        cmd => run_client(cmd).await,
    }
}
// --Daemon Mode---------------------------------------------------

async fn run_daemon() {
    let config = match Config::load() {
        Ok(c) => c,
        Err(e) => {
            eprint!("Failed to load config: {}", e);
            std::process::exit(1);
        }
    };

    let handle = match daemon::run(config).await {
        Ok(h) => h,
        Err(e) => {
            eprint!("Failed to load daemon: {}", e);
            std::process::exit(1);
        }
    };

    tokio::signal::ctrl_c().await.expect("failed to listen to ctrl_c");
    let _ = handle.shutdown(false).await;
}

//----- Client Mode ------------------------------------------------------------------

async fn run_client(cmd: Command){
    let mut conn = match client::connect(SOCKET_PATH, CONNECT_TIMEOUT).await {
        Ok(c) => c,
        Err(_) => {
            eprint!("Failed to connect to daemon. is it running?");
            std::process::exit(1);
        }
    };   

    let msg = match cmd {
        Command::Start { duration } =>{
            let seconds = match parse_time_to_seconds(&duration) {
                Ok(s) => s,
                Err(_) => {
                    eprint!("invalid duration '{}' use formats like 25m, 1h30m, 90s", duration);
                    std::process::exit(1);
                }
            };
            ClientMessage::Start { duration: seconds }
        }
        Command::Stop => ClientMessage::Stop,
        Command::Status => ClientMessage::GetStatus,
        Command::Daemon => unreachable!(),

    };

    send_and_handle(&mut conn, msg).await;
}

//rewrite match function later, alot of repeat code. 
async fn send_and_handle(conn: &mut Connection, msg: ClientMessage){
    let bytes = match protocols::encode(&msg){
        Ok(b) => b,
        Err(e) => {
            eprint!("Failed to encode message: {}", e);
            std::process::exit(1);
        }        
    };

    if let Err(e) =  conn.send(&bytes).await {
            eprint!("Failed to send: {}", e);
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
            eprintln!("failed to decode response: {}", e);
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
