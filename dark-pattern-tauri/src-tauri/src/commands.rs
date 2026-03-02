// src-tauri/src/commands.rs
//
// Tauri commands that wrap the daemon protocol.
// Each command connects to the daemon, sends a message, and returns the response.

use dark_pattern_blocker::client;
use dark_pattern_blocker::db::{BlockedApp, BlockedWebsite, DomRule, Profile, Schedule};
use dark_pattern_blocker::protocols::{ClientMessage, DaemonMessage};
use dark_pattern_blocker::socket::Connection;
use std::time::Duration;

const SOCKET_PATH: &str = "/tmp/dark-pattern.sock";
const TIMEOUT: Duration = Duration::from_secs(5);

// ─────────────────────────────────────────────────────────────────────────────
// Helper
// ─────────────────────────────────────────────────────────────────────────────

async fn connect() -> Result<Connection, String> {
    client::connect(SOCKET_PATH, TIMEOUT)
        .await
        .map_err(|_| "Failed to connect to daemon. Is it running?".to_string())
}

async fn send(msg: ClientMessage) -> Result<DaemonMessage, String> {
    let mut conn = connect().await?;
    client::send_and_receive(&mut conn, msg)
        .await
        .map_err(|e| e.to_string())
}

// ─────────────────────────────────────────────────────────────────────────────
// Session Commands
// ─────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_status() -> Result<DaemonMessage, String> {
    send(ClientMessage::GetStatus).await
}

#[tauri::command]
pub async fn start_session(duration: u64) -> Result<DaemonMessage, String> {
    send(ClientMessage::Start { duration }).await
}

#[tauri::command]
pub async fn stop_session() -> Result<DaemonMessage, String> {
    send(ClientMessage::Stop).await
}

// ─────────────────────────────────────────────────────────────────────────────
// Global Blocked Websites
// ─────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn list_global_websites() -> Result<DaemonMessage, String> {
    send(ClientMessage::ListGlobalWebsites).await
}

#[tauri::command]
pub async fn add_global_website(domain: String) -> Result<DaemonMessage, String> {
    send(ClientMessage::AddGlobalWebsite { domain }).await
}

#[tauri::command]
pub async fn remove_global_website(domain: String) -> Result<DaemonMessage, String> {
    send(ClientMessage::RemoveGlobalWebsite { domain }).await
}

// ─────────────────────────────────────────────────────────────────────────────
// Profiles
// ─────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn create_profile(name: String) -> Result<DaemonMessage, String> {
    send(ClientMessage::CreateProfile { name }).await
}

#[tauri::command]
pub async fn list_profiles() -> Result<DaemonMessage, String> {
    send(ClientMessage::ListProfiles).await
}

#[tauri::command]
pub async fn update_profile(id: String, name: String, enabled: bool) -> Result<DaemonMessage, String> {
    send(ClientMessage::UpdateProfile {
        profile: Profile {
            id,
            name,
            enabled,
        },
    })
    .await
}

#[tauri::command]
pub async fn delete_profile(id: String) -> Result<DaemonMessage, String> {
    send(ClientMessage::DeleteProfile { id }).await
}

// ─────────────────────────────────────────────────────────────────────────────
// Schedules
// ─────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn list_schedules(profile_id: String) -> Result<DaemonMessage, String> {
    send(ClientMessage::ListSchedules { profile_id }).await
}

#[tauri::command]
pub async fn create_schedule(
    profile_id: String,
    days: Vec<String>,
    start_time: String,
    end_time: String,
) -> Result<DaemonMessage, String> {
    send(ClientMessage::CreateSchedule {
        schedule: Schedule {
            id: 0,
            profile_id,
            schedule_type: None,
            days: Some(days),
            start_time: Some(start_time),
            end_time: Some(end_time),
            timezone: None,
            one_time_date: None,
        },
    })
    .await
}

#[tauri::command]
pub async fn delete_schedule(id: i64) -> Result<DaemonMessage, String> {
    send(ClientMessage::DeleteSchedule { id }).await
}

// ─────────────────────────────────────────────────────────────────────────────
// Blocked Websites (per profile)
// ─────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn list_blocked_websites(profile_id: String) -> Result<DaemonMessage, String> {
    send(ClientMessage::ListBlockedWebsites { profile_id }).await
}

#[tauri::command]
pub async fn add_blocked_website(profile_id: String, domain: String) -> Result<DaemonMessage, String> {
    send(ClientMessage::CreateBlockedWebsite {
        website: BlockedWebsite {
            id: 0,
            profile_id,
            domain,
        },
    })
    .await
}

#[tauri::command]
pub async fn delete_blocked_website(id: i64) -> Result<DaemonMessage, String> {
    send(ClientMessage::DeleteBlockedWebsite { id }).await
}

// ─────────────────────────────────────────────────────────────────────────────
// Blocked Apps (per profile)
// ─────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn list_blocked_apps(profile_id: String) -> Result<DaemonMessage, String> {
    send(ClientMessage::ListBlockedApps { profile_id }).await
}

#[tauri::command]
pub async fn add_blocked_app(profile_id: String, app_identifier: String) -> Result<DaemonMessage, String> {
    send(ClientMessage::CreateBlockedApp {
        app: BlockedApp {
            id: 0,
            profile_id,
            app_identifier,
        },
    })
    .await
}

#[tauri::command]
pub async fn delete_blocked_app(id: i64) -> Result<DaemonMessage, String> {
    send(ClientMessage::DeleteBlockedApp { id }).await
}

// ─────────────────────────────────────────────────────────────────────────────
// DOM Rules (per profile)
// ─────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn list_dom_rules(profile_id: String) -> Result<DaemonMessage, String> {
    send(ClientMessage::ListDomRules { profile_id }).await
}

#[tauri::command]
pub async fn add_dom_rule(profile_id: String, site: String, toggle: String) -> Result<DaemonMessage, String> {
    send(ClientMessage::CreateDomRule {
        rule: DomRule {
            id: 0,
            profile_id,
            site,
            toggle,
        },
    })
    .await
}

#[tauri::command]
pub async fn delete_dom_rule(id: i64) -> Result<DaemonMessage, String> {
    send(ClientMessage::DeleteDomRule { id }).await
}

// ─────────────────────────────────────────────────────────────────────────────
// Active Policy
// ─────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_active_policy() -> Result<DaemonMessage, String> {
    let now = chrono::Local::now();
    let current_day = now.format("%a").to_string().to_lowercase();
    let current_time = now.format("%H:%M").to_string();
    send(ClientMessage::GetActivePolicy { current_day, current_time }).await
}
