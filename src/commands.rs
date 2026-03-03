// src/commands.rs
//
// Tauri commands that wrap the daemon protocol.
// Each command calls client::send_message() and returns structured JSON.

use serde_json::{json, Value};

use crate::client;
use crate::db::{BlockedApp, BlockedWebsite, DomRule, Profile, Schedule};
use crate::format_duration::format_duration;
use crate::parse_duration::parse_time_to_seconds;
use crate::protocols::{ClientMessage, DaemonMessage};

// ─────────────────────────────────────────────────────────────────────────────
// Session
// ─────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn start_session(duration: String) -> Result<Value, String> {
    let seconds = parse_time_to_seconds(&duration)
        .map_err(|_| format!("Invalid duration '{}'. Use formats like 25m, 1h30m, 90s", duration))?;

    let response = client::send_message(ClientMessage::Start { duration: seconds }).await?;

    match response {
        DaemonMessage::Started { duration } => Ok(json!({
            "duration": duration,
            "formatted": format_duration(duration),
        })),
        other => Err(format!("Unexpected response: {:?}", other)),
    }
}

#[tauri::command]
pub async fn stop_session() -> Result<Value, String> {
    let response = client::send_message(ClientMessage::Stop).await?;

    match response {
        DaemonMessage::Stopped => Ok(json!({ "stopped": true })),
        other => Err(format!("Unexpected response: {:?}", other)),
    }
}

#[tauri::command]
pub async fn get_status() -> Result<Value, String> {
    let response = client::send_message(ClientMessage::GetStatus).await?;

    match response {
        DaemonMessage::StatusIdle { sites } => Ok(json!({
            "active": false,
            "sites": sites,
        })),
        DaemonMessage::StatusWithTime { time_left, sites } => Ok(json!({
            "active": true,
            "time_left": time_left,
            "formatted": format_duration(time_left),
            "sites": sites,
        })),
        other => Err(format!("Unexpected response: {:?}", other)),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Global Blocked Websites
// ─────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn add_global_website(domain: String) -> Result<Value, String> {
    let response = client::send_message(ClientMessage::AddGlobalWebsite { domain }).await?;

    match response {
        DaemonMessage::GlobalWebsiteAdded(ok) => Ok(json!({ "added": ok })),
        other => Err(format!("Unexpected response: {:?}", other)),
    }
}

#[tauri::command]
pub async fn remove_global_website(domain: String) -> Result<Value, String> {
    let response = client::send_message(ClientMessage::RemoveGlobalWebsite { domain }).await?;

    match response {
        DaemonMessage::GlobalWebsiteRemoved(ok) => Ok(json!({ "removed": ok })),
        other => Err(format!("Unexpected response: {:?}", other)),
    }
}

#[tauri::command]
pub async fn list_global_websites() -> Result<Value, String> {
    let response = client::send_message(ClientMessage::ListGlobalWebsites).await?;

    match response {
        DaemonMessage::GlobalWebsiteList(sites) => Ok(json!(sites)),
        other => Err(format!("Unexpected response: {:?}", other)),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Profiles
// ─────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn create_profile(name: String) -> Result<Value, String> {
    let response = client::send_message(ClientMessage::CreateProfile { name }).await?;

    match response {
        DaemonMessage::Profile(p) => Ok(json!({
            "id": p.id,
            "name": p.name,
            "enabled": p.enabled,
        })),
        other => Err(format!("Unexpected response: {:?}", other)),
    }
}

#[tauri::command]
pub async fn list_profiles() -> Result<Value, String> {
    let response = client::send_message(ClientMessage::ListProfiles).await?;

    match response {
        DaemonMessage::ProfileList(profiles) => {
            let list: Vec<Value> = profiles
                .into_iter()
                .map(|p| json!({ "id": p.id, "name": p.name, "enabled": p.enabled }))
                .collect();
            Ok(json!(list))
        }
        other => Err(format!("Unexpected response: {:?}", other)),
    }
}

#[tauri::command]
pub async fn update_profile(id: String, name: String, enabled: bool) -> Result<Value, String> {
    let response = client::send_message(ClientMessage::UpdateProfile {
        profile: Profile { id, name, enabled },
    })
    .await?;

    match response {
        DaemonMessage::ProfileUpdated => Ok(json!({ "updated": true })),
        other => Err(format!("Unexpected response: {:?}", other)),
    }
}

#[tauri::command]
pub async fn delete_profile(id: String) -> Result<Value, String> {
    let response = client::send_message(ClientMessage::DeleteProfile { id }).await?;

    match response {
        DaemonMessage::ProfileDeleted => Ok(json!({ "deleted": true })),
        other => Err(format!("Unexpected response: {:?}", other)),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Schedules
// ─────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn create_schedule(
    profile_id: String,
    days: String,
    start_time: String,
    end_time: String,
) -> Result<Value, String> {
    let days_vec: Vec<String> = days
        .split(',')
        .map(|d| d.trim().to_lowercase())
        .filter(|d| !d.is_empty())
        .collect();

    let response = client::send_message(ClientMessage::CreateSchedule {
        schedule: Schedule {
            id: 0,
            profile_id,
            schedule_type: None,
            days: Some(days_vec),
            start_time: Some(start_time),
            end_time: Some(end_time),
            timezone: None,
            one_time_date: None,
        },
    })
    .await?;

    match response {
        DaemonMessage::Schedule(s) => Ok(json!({
            "id": s.id,
            "profile_id": s.profile_id,
            "schedule_type": s.schedule_type,
            "days": s.days,
            "start_time": s.start_time,
            "end_time": s.end_time,
        })),
        other => Err(format!("Unexpected response: {:?}", other)),
    }
}

#[tauri::command]
pub async fn list_schedules(profile_id: String) -> Result<Value, String> {
    let response = client::send_message(ClientMessage::ListSchedules { profile_id }).await?;

    match response {
        DaemonMessage::ScheduleList(schedules) => {
            let list: Vec<Value> = schedules
                .into_iter()
                .map(|s| json!({
                    "id": s.id,
                    "profile_id": s.profile_id,
                    "schedule_type": s.schedule_type,
                    "days": s.days,
                    "start_time": s.start_time,
                    "end_time": s.end_time,
                }))
                .collect();
            Ok(json!(list))
        }
        other => Err(format!("Unexpected response: {:?}", other)),
    }
}

#[tauri::command]
pub async fn delete_schedule(id: i64) -> Result<Value, String> {
    let response = client::send_message(ClientMessage::DeleteSchedule { id }).await?;

    match response {
        DaemonMessage::ScheduleDeleted => Ok(json!({ "deleted": true })),
        other => Err(format!("Unexpected response: {:?}", other)),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Blocked Websites (per profile)
// ─────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn add_blocked_website(profile_id: String, domain: String) -> Result<Value, String> {
    let response = client::send_message(ClientMessage::CreateBlockedWebsite {
        website: BlockedWebsite {
            id: 0,
            profile_id,
            domain,
        },
    })
    .await?;

    match response {
        DaemonMessage::BlockedWebsite(w) => Ok(json!({
            "id": w.id,
            "profile_id": w.profile_id,
            "domain": w.domain,
        })),
        other => Err(format!("Unexpected response: {:?}", other)),
    }
}

#[tauri::command]
pub async fn list_blocked_websites(profile_id: String) -> Result<Value, String> {
    let response = client::send_message(ClientMessage::ListBlockedWebsites { profile_id }).await?;

    match response {
        DaemonMessage::BlockedWebsiteList(websites) => {
            let list: Vec<Value> = websites
                .into_iter()
                .map(|w| json!({
                    "id": w.id,
                    "profile_id": w.profile_id,
                    "domain": w.domain,
                }))
                .collect();
            Ok(json!(list))
        }
        other => Err(format!("Unexpected response: {:?}", other)),
    }
}

#[tauri::command]
pub async fn delete_blocked_website(id: i64) -> Result<Value, String> {
    let response = client::send_message(ClientMessage::DeleteBlockedWebsite { id }).await?;

    match response {
        DaemonMessage::BlockedWebsiteDeleted => Ok(json!({ "deleted": true })),
        other => Err(format!("Unexpected response: {:?}", other)),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Blocked Apps (per profile)
// ─────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn add_blocked_app(profile_id: String, app_identifier: String) -> Result<Value, String> {
    let response = client::send_message(ClientMessage::CreateBlockedApp {
        app: BlockedApp {
            id: 0,
            profile_id,
            app_identifier,
        },
    })
    .await?;

    match response {
        DaemonMessage::BlockedApp(a) => Ok(json!({
            "id": a.id,
            "profile_id": a.profile_id,
            "app_identifier": a.app_identifier,
        })),
        other => Err(format!("Unexpected response: {:?}", other)),
    }
}

#[tauri::command]
pub async fn list_blocked_apps(profile_id: String) -> Result<Value, String> {
    let response = client::send_message(ClientMessage::ListBlockedApps { profile_id }).await?;

    match response {
        DaemonMessage::BlockedAppList(apps) => {
            let list: Vec<Value> = apps
                .into_iter()
                .map(|a| json!({
                    "id": a.id,
                    "profile_id": a.profile_id,
                    "app_identifier": a.app_identifier,
                }))
                .collect();
            Ok(json!(list))
        }
        other => Err(format!("Unexpected response: {:?}", other)),
    }
}

#[tauri::command]
pub async fn delete_blocked_app(id: i64) -> Result<Value, String> {
    let response = client::send_message(ClientMessage::DeleteBlockedApp { id }).await?;

    match response {
        DaemonMessage::BlockedAppDeleted => Ok(json!({ "deleted": true })),
        other => Err(format!("Unexpected response: {:?}", other)),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// DOM Rules (per profile)
// ─────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn add_dom_rule(profile_id: String, site: String, toggle: String) -> Result<Value, String> {
    let response = client::send_message(ClientMessage::CreateDomRule {
        rule: DomRule {
            id: 0,
            profile_id,
            site,
            toggle,
        },
    })
    .await?;

    match response {
        DaemonMessage::DomRule(r) => Ok(json!({
            "id": r.id,
            "profile_id": r.profile_id,
            "site": r.site,
            "toggle": r.toggle,
        })),
        other => Err(format!("Unexpected response: {:?}", other)),
    }
}

#[tauri::command]
pub async fn list_dom_rules(profile_id: String) -> Result<Value, String> {
    let response = client::send_message(ClientMessage::ListDomRules { profile_id }).await?;

    match response {
        DaemonMessage::DomRuleList(rules) => {
            let list: Vec<Value> = rules
                .into_iter()
                .map(|r| json!({
                    "id": r.id,
                    "profile_id": r.profile_id,
                    "site": r.site,
                    "toggle": r.toggle,
                }))
                .collect();
            Ok(json!(list))
        }
        other => Err(format!("Unexpected response: {:?}", other)),
    }
}

#[tauri::command]
pub async fn delete_dom_rule(id: i64) -> Result<Value, String> {
    let response = client::send_message(ClientMessage::DeleteDomRule { id }).await?;

    match response {
        DaemonMessage::DomRuleDeleted => Ok(json!({ "deleted": true })),
        other => Err(format!("Unexpected response: {:?}", other)),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Active Policy
// ─────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_active_policy() -> Result<Value, String> {
    let now = chrono::Local::now();
    let current_day = now.format("%a").to_string().to_lowercase();
    let current_time = now.format("%H:%M").to_string();

    let response = client::send_message(ClientMessage::GetActivePolicy {
        current_day,
        current_time,
    })
    .await?;

    match response {
        DaemonMessage::ActivePolicy(p) => Ok(json!({
            "blocked_websites": p.blocked_websites,
            "blocked_apps": p.blocked_apps,
            "dom_rules": p.dom_rules.into_iter().map(|r| json!({
                "id": r.id,
                "site": r.site,
                "toggle": r.toggle,
            })).collect::<Vec<_>>(),
        })),
        other => Err(format!("Unexpected response: {:?}", other)),
    }
}
