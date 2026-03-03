// src/bin/native-host.rs
//
// Native Messaging Host for Chrome/Firefox extensions.
// Bridges the extension to the focusd daemon via Unix socket.
//
// Protocol:
//   Extension <-> Host: JSON with 4-byte little-endian length prefix (native messaging spec)
//   Host <-> Daemon: MessagePack with 4-byte big-endian length prefix (LengthDelimitedCodec)

use dark_pattern_blocker::config::config::Config;
use dark_pattern_blocker::daemon;
use dark_pattern_blocker::client;
use dark_pattern_blocker::protocols::{ClientMessage, DaemonMessage};
use std::time::Duration;
use tempfile::tempdir;

use std::io::{self, Read, Write};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use dark_pattern_blocker::db::{
    ActivePolicy, BlockedApp, BlockedWebsite, DomRule, ManualSession, Profile, Schedule,
};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

const SOCKET_PATH: &str = "/tmp/dark-pattern.sock";
const LOG_PATH: &str = "/tmp/native-host.log";

// ─────────────────────────────────────────────────────────────────────────────
// Extension Message Types (JSON, for native messaging)
//
// These are the JSON envelope types the browser extension sends/receives.
// They translate 1:1 to/from the daemon's ClientMessage/DaemonMessage.
// ─────────────────────────────────────────────────────────────────────────────

/// Messages FROM the extension (JSON with SCREAMING_SNAKE_CASE tags)
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
enum ExtensionMessage {
    Ping,
    Start { duration: u64 },
    Stop,
    GetStatus,

    // Profiles
    CreateProfile { name: String },
    GetProfile { id: String },
    ListProfiles,
    UpdateProfile { profile: Profile },
    DeleteProfile { id: String },

    // Schedules
    CreateSchedule { schedule: Schedule },
    GetSchedule { id: i64 },
    ListSchedules { profile_id: String },
    UpdateSchedule { schedule: Schedule },
    DeleteSchedule { id: i64 },

    // Blocked Websites
    CreateBlockedWebsite { website: BlockedWebsite },
    ListBlockedWebsites { profile_id: String },
    DeleteBlockedWebsite { id: i64 },

    // Blocked Apps
    CreateBlockedApp { app: BlockedApp },
    GetBlockedApp { id: i64 },
    ListBlockedApps { profile_id: String },
    DeleteBlockedApp { id: i64 },

    // DOM Rules
    CreateDomRule { rule: DomRule },
    GetDomRule { id: i64 },
    ListDomRules { profile_id: String },
    DeleteDomRule { id: i64 },

    // Manual Sessions
    CreateManualSession { session: ManualSession },
    GetManualSession { id: i64 },
    GetActiveManualSession,
    ListManualSessions,
    UpdateManualSession { session: ManualSession },

    // Global Blocked Websites
    AddGlobalWebsite { domain: String },
    RemoveGlobalWebsite { domain: String },
    ListGlobalWebsites,

    // Active Policy
    GetActivePolicy { current_day: String, current_time: String },
}

/// Messages TO the extension (JSON with camelCase tags)
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum ExtensionResponse {
    Pong,
    Started { duration: u64 },
    Stopped,
    Status { active: bool, time_left: Option<u64>, sites: Vec<String> },
    Error { message: String },

    // Profiles
    Profile { profile: Profile },
    ProfileList { profiles: Vec<Profile> },
    ProfileUpdated,
    ProfileDeleted,

    // Schedules
    Schedule { schedule: Schedule },
    ScheduleList { schedules: Vec<Schedule> },
    ScheduleUpdated,
    ScheduleDeleted,

    // Blocked Websites
    BlockedWebsite { website: BlockedWebsite },
    BlockedWebsiteList { websites: Vec<BlockedWebsite> },
    BlockedWebsiteDeleted,

    // Blocked Apps
    BlockedApp { app: BlockedApp },
    BlockedAppList { apps: Vec<BlockedApp> },
    BlockedAppDeleted,

    // DOM Rules
    DomRule { rule: DomRule },
    DomRuleList { rules: Vec<DomRule> },
    DomRuleDeleted,

    // Manual Sessions
    ManualSession { session: Option<ManualSession> },
    ManualSessionList { sessions: Vec<ManualSession> },
    ManualSessionUpdated,

    // Global Blocked Websites
    GlobalWebsiteAdded { success: bool },
    GlobalWebsiteRemoved { success: bool },
    GlobalWebsiteList { websites: Vec<String> },

    // Active Policy / Push
    ActivePolicy { policy: ActivePolicy },
}

// ─────────────────────────────────────────────────────────────────────────────
// Logging (debug only)
// ─────────────────────────────────────────────────────────────────────────────

fn log(msg: &str) {
    use std::fs::OpenOptions;
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(LOG_PATH)
    {
        let timestamp = chrono::Local::now().format("%H:%M:%S%.3f");
        let _ = writeln!(file, "[{}] {}", timestamp, msg);
    }
}

macro_rules! log {
    ($($arg:tt)*) => {
        log(&format!($($arg)*))
    };
}

// ─────────────────────────────────────────────────────────────────────────────
// Native Messaging Protocol (Extension <-> Host)
// Uses little-endian 4-byte length prefix + JSON
// ─────────────────────────────────────────────────────────────────────────────

/// Read a native messaging message from stdin (blocking)
fn read_native_message() -> Result<Vec<u8>> {
    let mut len_bytes = [0u8; 4];
    io::stdin()
        .read_exact(&mut len_bytes)
        .context("Failed to read message length")?;

    let len = u32::from_le_bytes(len_bytes) as usize;
    log!("Reading message of {} bytes", len);

    if len > 1024 * 1024 {
        bail!("Message too large: {} bytes", len);
    }

    let mut buffer = vec![0u8; len];
    io::stdin()
        .read_exact(&mut buffer)
        .context("Failed to read message body")?;

    Ok(buffer)
}

/// Write a native messaging message to stdout (blocking, run on threadpool)
async fn write_native_message(msg: Vec<u8>) -> Result<()> {
    tokio::task::spawn_blocking(move || -> Result<()> {
        let len = (msg.len() as u32).to_le_bytes();
        let mut stdout = io::stdout().lock();

        stdout.write_all(&len)?;
        stdout.write_all(&msg)?;
        stdout.flush()?;

        log!("Wrote {} bytes to extension", msg.len());
        Ok(())
    })
    .await
    .context("write task panicked")?
}

// ─────────────────────────────────────────────────────────────────────────────
// Daemon Protocol (Host <-> Daemon)
// Uses big-endian 4-byte length prefix + MessagePack (LengthDelimitedCodec)
// ─────────────────────────────────────────────────────────────────────────────

async fn send_to_daemon(stream: &mut UnixStream, msg: &ClientMessage) -> Result<()> {
    let encoded = rmp_serde::to_vec(msg)
        .context("Failed to encode message for daemon")?;

    let len = (encoded.len() as u32).to_be_bytes();

    stream.write_all(&len).await?;
    stream.write_all(&encoded).await?;
    stream.flush().await?;

    log!("Sent {} bytes to daemon", encoded.len());
    Ok(())
}

async fn recv_from_daemon(stream: &mut UnixStream) -> Result<DaemonMessage> {
    let mut len_bytes = [0u8; 4];
    stream.read_exact(&mut len_bytes).await
        .context("Failed to read daemon response length")?;

    let len = u32::from_be_bytes(len_bytes) as usize;
    log!("Reading {} bytes from daemon", len);

    if len > 1024 * 1024 {
        bail!("Daemon response too large: {} bytes", len);
    }

    let mut buffer = vec![0u8; len];
    stream.read_exact(&mut buffer).await
        .context("Failed to read daemon response body")?;

    rmp_serde::from_slice(&buffer)
        .context("Failed to decode daemon response")
}

// ─────────────────────────────────────────────────────────────────────────────
// Message Translation
// ─────────────────────────────────────────────────────────────────────────────

/// Convert an extension JSON message to the daemon's ClientMessage.
fn extension_to_client(msg: ExtensionMessage) -> ClientMessage {
    match msg {
        ExtensionMessage::Ping => ClientMessage::Ping,
        ExtensionMessage::Start { duration } => ClientMessage::Start { duration },
        ExtensionMessage::Stop => ClientMessage::Stop,
        ExtensionMessage::GetStatus => ClientMessage::GetStatus,

        // Profiles
        ExtensionMessage::CreateProfile { name } => ClientMessage::CreateProfile { name },
        ExtensionMessage::GetProfile { id } => ClientMessage::GetProfile { id },
        ExtensionMessage::ListProfiles => ClientMessage::ListProfiles,
        ExtensionMessage::UpdateProfile { profile } => ClientMessage::UpdateProfile { profile },
        ExtensionMessage::DeleteProfile { id } => ClientMessage::DeleteProfile { id },

        // Schedules
        ExtensionMessage::CreateSchedule { schedule } => ClientMessage::CreateSchedule { schedule },
        ExtensionMessage::GetSchedule { id } => ClientMessage::GetSchedule { id },
        ExtensionMessage::ListSchedules { profile_id } => ClientMessage::ListSchedules { profile_id },
        ExtensionMessage::UpdateSchedule { schedule } => ClientMessage::UpdateSchedule { schedule },
        ExtensionMessage::DeleteSchedule { id } => ClientMessage::DeleteSchedule { id },

        // Blocked Websites
        ExtensionMessage::CreateBlockedWebsite { website } => ClientMessage::CreateBlockedWebsite { website },
        ExtensionMessage::ListBlockedWebsites { profile_id } => ClientMessage::ListBlockedWebsites { profile_id },
        ExtensionMessage::DeleteBlockedWebsite { id } => ClientMessage::DeleteBlockedWebsite { id },

        // Blocked Apps
        ExtensionMessage::CreateBlockedApp { app } => ClientMessage::CreateBlockedApp { app },
        ExtensionMessage::GetBlockedApp { id } => ClientMessage::GetBlockedApp { id },
        ExtensionMessage::ListBlockedApps { profile_id } => ClientMessage::ListBlockedApps { profile_id },
        ExtensionMessage::DeleteBlockedApp { id } => ClientMessage::DeleteBlockedApp { id },

        // DOM Rules
        ExtensionMessage::CreateDomRule { rule } => ClientMessage::CreateDomRule { rule },
        ExtensionMessage::GetDomRule { id } => ClientMessage::GetDomRule { id },
        ExtensionMessage::ListDomRules { profile_id } => ClientMessage::ListDomRules { profile_id },
        ExtensionMessage::DeleteDomRule { id } => ClientMessage::DeleteDomRule { id },

        // Manual Sessions
        ExtensionMessage::CreateManualSession { session } => ClientMessage::CreateManualSession { session },
        ExtensionMessage::GetManualSession { id } => ClientMessage::GetManualSession { id },
        ExtensionMessage::GetActiveManualSession => ClientMessage::GetActiveManualSession,
        ExtensionMessage::ListManualSessions => ClientMessage::ListManualSessions,
        ExtensionMessage::UpdateManualSession { session } => ClientMessage::UpdateManualSession { session },

        // Global Blocked Websites
        ExtensionMessage::AddGlobalWebsite { domain } => ClientMessage::AddGlobalWebsite { domain },
        ExtensionMessage::RemoveGlobalWebsite { domain } => ClientMessage::RemoveGlobalWebsite { domain },
        ExtensionMessage::ListGlobalWebsites => ClientMessage::ListGlobalWebsites,

        // Active Policy
        ExtensionMessage::GetActivePolicy { current_day, current_time } => {
            ClientMessage::GetActivePolicy { current_day, current_time }
        }
    }
}

/// Convert a daemon DaemonMessage to a JSON-friendly ExtensionResponse.
fn daemon_to_extension(msg: DaemonMessage) -> ExtensionResponse {
    match msg {
        DaemonMessage::Pong => ExtensionResponse::Pong,
        DaemonMessage::Started { duration } => ExtensionResponse::Started { duration },
        DaemonMessage::Stopped => ExtensionResponse::Stopped,
        DaemonMessage::StatusWithTime { time_left, sites } => ExtensionResponse::Status {
            active: true,
            time_left: Some(time_left),
            sites,
        },
        DaemonMessage::StatusIdle { sites } => ExtensionResponse::Status {
            active: false,
            time_left: None,
            sites,
        },
        DaemonMessage::Error(message) => ExtensionResponse::Error { message },

        // Profiles
        DaemonMessage::Profile(profile) => ExtensionResponse::Profile { profile },
        DaemonMessage::ProfileList(profiles) => ExtensionResponse::ProfileList { profiles },
        DaemonMessage::ProfileUpdated => ExtensionResponse::ProfileUpdated,
        DaemonMessage::ProfileDeleted => ExtensionResponse::ProfileDeleted,

        // Schedules
        DaemonMessage::Schedule(schedule) => ExtensionResponse::Schedule { schedule },
        DaemonMessage::ScheduleList(schedules) => ExtensionResponse::ScheduleList { schedules },
        DaemonMessage::ScheduleUpdated => ExtensionResponse::ScheduleUpdated,
        DaemonMessage::ScheduleDeleted => ExtensionResponse::ScheduleDeleted,

        // Blocked Websites
        DaemonMessage::BlockedWebsite(website) => ExtensionResponse::BlockedWebsite { website },
        DaemonMessage::BlockedWebsiteList(websites) => ExtensionResponse::BlockedWebsiteList { websites },
        DaemonMessage::BlockedWebsiteDeleted => ExtensionResponse::BlockedWebsiteDeleted,

        // Blocked Apps
        DaemonMessage::BlockedApp(app) => ExtensionResponse::BlockedApp { app },
        DaemonMessage::BlockedAppList(apps) => ExtensionResponse::BlockedAppList { apps },
        DaemonMessage::BlockedAppDeleted => ExtensionResponse::BlockedAppDeleted,

        // DOM Rules
        DaemonMessage::DomRule(rule) => ExtensionResponse::DomRule { rule },
        DaemonMessage::DomRuleList(rules) => ExtensionResponse::DomRuleList { rules },
        DaemonMessage::DomRuleDeleted => ExtensionResponse::DomRuleDeleted,

        // Manual Sessions
        DaemonMessage::ManualSession(session) => ExtensionResponse::ManualSession { session },
        DaemonMessage::ManualSessionList(sessions) => ExtensionResponse::ManualSessionList { sessions },
        DaemonMessage::ManualSessionUpdated => ExtensionResponse::ManualSessionUpdated,

        // Global Blocked Websites
        DaemonMessage::GlobalWebsiteAdded(success) => ExtensionResponse::GlobalWebsiteAdded { success },
        DaemonMessage::GlobalWebsiteRemoved(success) => ExtensionResponse::GlobalWebsiteRemoved { success },
        DaemonMessage::GlobalWebsiteList(websites) => ExtensionResponse::GlobalWebsiteList { websites },

        // Active Policy
        DaemonMessage::ActivePolicy(policy) => ExtensionResponse::ActivePolicy { policy },

        // Push notifications — same shape as ActivePolicy for the extension
        DaemonMessage::PolicyChanged(policy) => ExtensionResponse::ActivePolicy { policy },

        // Subscription ack — extension doesn't need to see this
        DaemonMessage::Subscribed => ExtensionResponse::Pong,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Extension IO helpers
// ─────────────────────────────────────────────────────────────────────────────

async fn read_from_extension() -> Result<ExtensionMessage> {
    tokio::task::spawn_blocking(|| {
        let bytes = read_native_message()?;
        serde_json::from_slice(&bytes).context("Failed to parse extension message")
    })
    .await
    .context("Read task panicked")?
}

async fn write_to_extension(response: &ExtensionResponse) -> Result<()> {
    let json = serde_json::to_vec(response)
        .context("Failed to serialize extension response")?;
    write_native_message(json).await
}

// ─────────────────────────────────────────────────────────────────────────────
// Request/Response
// ─────────────────────────────────────────────────────────────────────────────

async fn handle_message(stream: &mut UnixStream, ext_msg: ExtensionMessage) -> Result<ExtensionResponse> {
    let client_msg = extension_to_client(ext_msg);
    log!("   ... converted to client message: {:?}", &client_msg);

    send_to_daemon(stream, &client_msg)
        .await
        .context("Failed to send message to daemon")?;
    log!("   ... sent to daemon, awaiting response");

    let daemon_msg = recv_from_daemon(stream)
        .await
        .context("Failed to receive message from daemon")?;
    log!("   ... received from daemon: {:?}", &daemon_msg);

    let response = daemon_to_extension(daemon_msg);
    log!("   ... converted to extension response: {:?}", &response);

    Ok(response)
}

// ─────────────────────────────────────────────────────────────────────────────
// Main Loop
// ─────────────────────────────────────────────────────────────────────────────

async fn run() -> Result<()> {
    log!("=== Native host starting ===");

    // Retry connecting to the daemon with backoff.
    // The daemon might not be running yet (e.g., system just booted).
    // The native host stays alive and keeps trying so the extension
    // doesn't get stuck in a reconnect loop.
    let mut stream = loop {
        match UnixStream::connect(SOCKET_PATH).await {
            Ok(s) => break s,
            Err(e) => {
                log!("Daemon not available ({}), retrying in 5s...", e);

                // Tell the extension we're still waiting
                let response = ExtensionResponse::Error {
                    message: "Waiting for daemon...".to_string(),
                };
                // Best-effort — extension might not be listening yet
                let _ = write_to_extension(&response).await;

                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        }
    };
    log!("Connected to daemon");

    // Subscribe to policy changes
    send_to_daemon(&mut stream, &ClientMessage::SubscribePolicyChanges)
        .await
        .context("Failed to send subscribe message")?;

    // Wait for subscription ack
    let ack = recv_from_daemon(&mut stream).await
        .context("Failed to receive subscription ack")?;
    log!("Subscription ack: {:?}", ack);

    // Request the current active policy so the extension gets the full picture
    // (blocked websites, blocked apps, DOM rules) immediately on connect.
    {
        let now = chrono::Local::now();
        let policy_req = ClientMessage::GetActivePolicy {
            current_day: now.format("%a").to_string().to_lowercase(),
            current_time: now.format("%H:%M").to_string(),
        };
        send_to_daemon(&mut stream, &policy_req).await
            .context("Failed to request initial policy")?;

        let policy_msg = recv_from_daemon(&mut stream).await
            .context("Failed to receive initial policy")?;
        log!("Initial policy: {:?}", policy_msg);

        let response = daemon_to_extension(policy_msg);
        write_to_extension(&response).await?;
    }

    // Main message loop — bidirectional:
    //   stdin (extension requests) → daemon → stdout (extension responses)
    //   daemon (policy pushes) → stdout (extension notifications)
    loop {
        tokio::select! {
            // Messages FROM extension (stdin → daemon)
            msg_result = read_from_extension() => {
                match msg_result {
                    Ok(ext_msg) => {
                        log!("Extension message: {:?}", ext_msg);
                        let response = handle_message(&mut stream, ext_msg).await?;
                        write_to_extension(&response).await?;
                    }
                    Err(e) => {
                        log!("Extension read error (closed?): {}", e);
                        break;
                    }
                }
            }

            // Push messages FROM daemon (policy changes → extension)
            daemon_result = recv_from_daemon(&mut stream) => {
                match daemon_result {
                    Ok(msg) => {
                        log!("Daemon push: {:?}", msg);
                        let response = daemon_to_extension(msg);
                        write_to_extension(&response).await?;
                    }
                    Err(e) => {
                        log!("Daemon connection lost: {}", e);
                        break;
                    }
                }
            }
        }
    }

    log!("=== Native host exiting ===");
    Ok(())
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    if let Err(e) = run().await {
        log!("Fatal error: {}", e);

        // Try to send error to extension
        let response = ExtensionResponse::Error {
            message: format!("Native host error: {}", e),
        };
        if let Ok(json) = serde_json::to_vec(&response) {
            let _ = write_native_message(json).await;
        }

        std::process::exit(1);
    }
}

#[tokio::test]
async fn native_host_receives_correct_active_policy() {
    // 1. Set up a daemon with a temp socket + temp DB
    let dir = tempdir().unwrap();
    let config = Config::from_toml(&format!(
        r#"
        hosts_file = "{}"
        socket_path = "{}"
        pid_path = "{}"
        db_path = "{}"
        "#,
        dir.path().join("hosts").display(),
        dir.path().join("test.sock").display(),
        dir.path().join("test.pid").display(),
        dir.path().join("test.db").display(),
    )).unwrap();

    let socket_path = dir.path().join("test.sock");
    let handle = daemon::run(config).await.unwrap();

    // 2. Connect as a client and set up test data
    let mut conn = client::connect(&socket_path, Duration::from_secs(5))
        .await.unwrap();

    // Create a profile
    let resp = client::send_and_receive(&mut conn, 
        ClientMessage::CreateProfile { name: "Test".into() }
    ).await.unwrap();
    
    let profile_id = match resp {
        DaemonMessage::Profile(p) => p.id,
        _ => panic!("Expected Profile"),
    };

    // Enable it (it's enabled by default, but be explicit)
    client::send_and_receive(&mut conn,
        ClientMessage::UpdateProfile {
            profile: Profile { id: profile_id.clone(), name: "Test".into(), enabled: true },
        }
    ).await.unwrap();

    // Add a DOM rule
    client::send_and_receive(&mut conn,
        ClientMessage::CreateDomRule {
            rule: DomRule { id: 0, profile_id: profile_id.clone(), 
                           site: "youtube".into(), toggle: "hideShorts".into() },
        }
    ).await.unwrap();

    // Add a blocked website
    client::send_and_receive(&mut conn,
        ClientMessage::CreateBlockedWebsite {
            website: BlockedWebsite { id: 0, profile_id: profile_id.clone(),
                                      domain: "reddit.com".into() },
        }
    ).await.unwrap();

    // 3. Query active policy (same as native-host does on startup)
    let now = chrono::Local::now();
    let resp = client::send_and_receive(&mut conn,
        ClientMessage::GetActivePolicy {
            current_day: now.format("%a").to_string().to_lowercase(),
            current_time: now.format("%H:%M").to_string(),
        }
    ).await.unwrap();

    // 4. Assert the policy contains our data
    match resp {
        DaemonMessage::ActivePolicy(policy) => {
            assert!(policy.blocked_websites.contains(&"reddit.com".to_string()),
                "Expected reddit.com in blocked_websites: {:?}", policy.blocked_websites);
            assert!(policy.dom_rules.iter().any(|r| r.site == "youtube" && r.toggle == "hideShorts"),
                "Expected youtube/hideShorts DOM rule: {:?}", policy.dom_rules);
        }
        other => panic!("Expected ActivePolicy, got {:?}", other),
    }

    // Cleanup
    let _ = handle.shutdown(false).await;
}