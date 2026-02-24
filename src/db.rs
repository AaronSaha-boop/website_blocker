// src/db.rs

use rusqlite::{params, Connection, Error as RusqliteError, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

// ─────────────────────────────────────────────────────────────────────────────
// Error
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Error, Debug)]
pub enum DbError {
    #[error("Database error: {0}")]
    Sqlite(String),

    #[error("JSON error: {0}")]
    Json(String),

    #[error("Database actor unavailable")]
    ActorGone,
}

impl From<rusqlite::Error> for DbError {
    fn from(e: rusqlite::Error) -> Self {
        DbError::Sqlite(e.to_string())
    }
}

impl From<serde_json::Error> for DbError {
    fn from(e: serde_json::Error) -> Self {
        DbError::Json(e.to_string())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Data types
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ScheduleType {
    #[serde(rename = "recurring")]
    Recurring,
    #[serde(rename = "one_time")]
    OneTime,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Schedule {
    pub id: i64,
    pub profile_id: String,
    pub schedule_type: Option<ScheduleType>,
    pub days: Option<Vec<String>>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub timezone: Option<String>,
    pub one_time_date: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlockedWebsite {
    pub id: i64,
    pub profile_id: String,
    pub domain: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlockedApp {
    pub id: i64,
    pub profile_id: String,
    pub app_identifier: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DomRule {
    pub id: i64,
    pub profile_id: String,
    pub site: String,
    pub toggle: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManualSession {
    pub id: i64,
    pub started_at: String,
    pub duration_secs: i64,
    pub ended_at: Option<String>,
    pub policy_json: Option<String>,
}

/// Merged policy from all active profiles + manual session
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ActivePolicy {
    pub blocked_websites: Vec<String>,
    pub blocked_apps: Vec<String>,
    pub dom_rules: Vec<DomRule>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Commands
// ─────────────────────────────────────────────────────────────────────────────

enum DbCommand {
    // Profiles
    CreateProfile {
        name: String,
        reply: oneshot::Sender<Result<Profile, DbError>>,
    },
    GetProfile {
        id: String,
        reply: oneshot::Sender<Result<Option<Profile>, DbError>>,
    },
    ListProfiles {
        reply: oneshot::Sender<Result<Vec<Profile>, DbError>>,
    },
    UpdateProfile {
        profile: Profile,
        reply: oneshot::Sender<Result<(), DbError>>,
    },
    DeleteProfile {
        id: String,
        reply: oneshot::Sender<Result<(), DbError>>,
    },

    // Schedules
    CreateSchedule {
        schedule: Schedule,
        reply: oneshot::Sender<Result<Schedule, DbError>>,
    },
    GetSchedule {
        id: i64,
        reply: oneshot::Sender<Result<Option<Schedule>, DbError>>,
    },
    ListSchedules {
        profile_id: String,
        reply: oneshot::Sender<Result<Vec<Schedule>, DbError>>,
    },
    UpdateSchedule {
        schedule: Schedule,
        reply: oneshot::Sender<Result<(), DbError>>,
    },
    DeleteSchedule {
        id: i64,
        reply: oneshot::Sender<Result<(), DbError>>,
    },

    // Blocked Websites (profile-specific)
    CreateBlockedWebsite {
        website: BlockedWebsite,
        reply: oneshot::Sender<Result<BlockedWebsite, DbError>>,
    },
    ListBlockedWebsites {
        profile_id: String,
        reply: oneshot::Sender<Result<Vec<BlockedWebsite>, DbError>>,
    },
    DeleteBlockedWebsite {
        id: i64,
        reply: oneshot::Sender<Result<(), DbError>>,
    },

    // Blocked Apps
    CreateBlockedApp {
        app: BlockedApp,
        reply: oneshot::Sender<Result<BlockedApp, DbError>>,
    },
    GetBlockedApp {
        id: i64,
        reply: oneshot::Sender<Result<Option<BlockedApp>, DbError>>,
    },
    ListBlockedApps {
        profile_id: String,
        reply: oneshot::Sender<Result<Vec<BlockedApp>, DbError>>,
    },
    DeleteBlockedApp {
        id: i64,
        reply: oneshot::Sender<Result<(), DbError>>,
    },

    // DOM Rules
    CreateDomRule {
        rule: DomRule,
        reply: oneshot::Sender<Result<DomRule, DbError>>,
    },
    GetDomRule {
        id: i64,
        reply: oneshot::Sender<Result<Option<DomRule>, DbError>>,
    },
    ListDomRules {
        profile_id: String,
        reply: oneshot::Sender<Result<Vec<DomRule>, DbError>>,
    },
    DeleteDomRule {
        id: i64,
        reply: oneshot::Sender<Result<(), DbError>>,
    },

    // Manual Sessions
    CreateManualSession {
        session: ManualSession,
        reply: oneshot::Sender<Result<ManualSession, DbError>>,
    },
    GetManualSession {
        id: i64,
        reply: oneshot::Sender<Result<Option<ManualSession>, DbError>>,
    },
    GetActiveManualSession {
        reply: oneshot::Sender<Result<Option<ManualSession>, DbError>>,
    },
    ListManualSessions {
        reply: oneshot::Sender<Result<Vec<ManualSession>, DbError>>,
    },
    UpdateManualSession {
        session: ManualSession,
        reply: oneshot::Sender<Result<(), DbError>>,
    },

    // Global blocked websites (profile-independent, for quick blocking)
    AddGlobalWebsite {
        domain: String,
        reply: oneshot::Sender<Result<bool, DbError>>,
    },
    RemoveGlobalWebsite {
        domain: String,
        reply: oneshot::Sender<Result<bool, DbError>>,
    },
    ListGlobalWebsites {
        reply: oneshot::Sender<Result<Vec<String>, DbError>>,
    },

    // Active profiles query
    GetActiveProfiles {
        current_day: String,
        current_time: String,
        reply: oneshot::Sender<Result<Vec<Profile>, DbError>>,
    },

    // Get merged active policy
    GetActivePolicy {
        current_day: String,
        current_time: String,
        reply: oneshot::Sender<Result<ActivePolicy, DbError>>,
    },
}

// ─────────────────────────────────────────────────────────────────────────────
// Handle (public API — Send + Clone)
// ─────────────────────────────────────────────────────────────────────────────

/// Clone-able, `Send` handle for async code to talk to the database.
///
/// The actual `rusqlite::Connection` lives on a dedicated OS thread;
/// this handle communicates with it over an MPSC channel.
#[derive(Clone)]
pub struct DbHandle {
    tx: mpsc::Sender<DbCommand>,
}

impl DbHandle {
    /// Open the database on a background thread and return a handle.
    pub fn open(path: &Path) -> Result<Self, DbError> {
        let (tx, rx) = mpsc::channel::<DbCommand>(64);

        // Open + migrate synchronously so callers know immediately if it fails.
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;
             PRAGMA busy_timeout = 5000;",
        )?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS profiles (
                 id TEXT PRIMARY KEY,
                 name TEXT NOT NULL,
                 enabled INTEGER DEFAULT 1
             );

             CREATE TABLE IF NOT EXISTS schedules (
                 id INTEGER PRIMARY KEY,
                 profile_id TEXT REFERENCES profiles(id) ON DELETE CASCADE,
                 type TEXT CHECK(type IN ('recurring', 'one_time')),
                 days TEXT,
                 start_time TEXT,
                 end_time TEXT,
                 timezone TEXT DEFAULT 'America/New_York',
                 one_time_date TEXT
             );

             CREATE TABLE IF NOT EXISTS blocked_websites (
                 id INTEGER PRIMARY KEY,
                 profile_id TEXT REFERENCES profiles(id) ON DELETE CASCADE,
                 domain TEXT NOT NULL
             );

             CREATE TABLE IF NOT EXISTS blocked_apps (
                 id INTEGER PRIMARY KEY,
                 profile_id TEXT REFERENCES profiles(id) ON DELETE CASCADE,
                 app_identifier TEXT NOT NULL
             );

             CREATE TABLE IF NOT EXISTS dom_rules (
                 id INTEGER PRIMARY KEY,
                 profile_id TEXT REFERENCES profiles(id) ON DELETE CASCADE,
                 site TEXT NOT NULL,
                 toggle TEXT NOT NULL
             );

             CREATE TABLE IF NOT EXISTS manual_sessions (
                 id INTEGER PRIMARY KEY,
                 started_at TEXT NOT NULL,
                 duration_secs INTEGER NOT NULL,
                 ended_at TEXT,
                 policy_json TEXT
             );

             CREATE TABLE IF NOT EXISTS global_blocked_websites (
                 id INTEGER PRIMARY KEY,
                 domain TEXT NOT NULL UNIQUE
             );

             CREATE INDEX IF NOT EXISTS idx_schedules_profile ON schedules(profile_id);
             CREATE INDEX IF NOT EXISTS idx_blocked_websites_profile ON blocked_websites(profile_id);
             CREATE INDEX IF NOT EXISTS idx_blocked_apps_profile ON blocked_apps(profile_id);
             CREATE INDEX IF NOT EXISTS idx_dom_rules_profile ON dom_rules(profile_id);",
        )?;

        // Move the connection onto a dedicated thread.
        std::thread::Builder::new()
            .name("db-actor".into())
            .spawn(move || db_thread(conn, rx))
            .map_err(|e| DbError::Sqlite(format!("failed to spawn db thread: {e}")))?;

        Ok(Self { tx })
    }

    async fn send_cmd<T>(
        &self,
        make_cmd: impl FnOnce(oneshot::Sender<Result<T, DbError>>) -> DbCommand,
    ) -> Result<T, DbError> {
        let (reply, rx) = oneshot::channel();
        self.tx
            .send(make_cmd(reply))
            .await
            .map_err(|_| DbError::ActorGone)?;
        rx.await.map_err(|_| DbError::ActorGone)?
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Profiles
    // ─────────────────────────────────────────────────────────────────────────

    pub async fn create_profile(&self, name: String) -> Result<Profile, DbError> {
        self.send_cmd(|reply| DbCommand::CreateProfile { name, reply })
            .await
    }

    pub async fn get_profile(&self, id: String) -> Result<Option<Profile>, DbError> {
        self.send_cmd(|reply| DbCommand::GetProfile { id, reply })
            .await
    }

    pub async fn list_profiles(&self) -> Result<Vec<Profile>, DbError> {
        self.send_cmd(|reply| DbCommand::ListProfiles { reply })
            .await
    }

    pub async fn update_profile(&self, profile: Profile) -> Result<(), DbError> {
        self.send_cmd(|reply| DbCommand::UpdateProfile { profile, reply })
            .await
    }

    pub async fn delete_profile(&self, id: String) -> Result<(), DbError> {
        self.send_cmd(|reply| DbCommand::DeleteProfile { id, reply })
            .await
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Schedules
    // ─────────────────────────────────────────────────────────────────────────

    pub async fn create_schedule(&self, schedule: Schedule) -> Result<Schedule, DbError> {
        self.send_cmd(|reply| DbCommand::CreateSchedule { schedule, reply })
            .await
    }

    pub async fn get_schedule(&self, id: i64) -> Result<Option<Schedule>, DbError> {
        self.send_cmd(|reply| DbCommand::GetSchedule { id, reply })
            .await
    }

    pub async fn list_schedules(&self, profile_id: String) -> Result<Vec<Schedule>, DbError> {
        self.send_cmd(|reply| DbCommand::ListSchedules { profile_id, reply })
            .await
    }

    pub async fn update_schedule(&self, schedule: Schedule) -> Result<(), DbError> {
        self.send_cmd(|reply| DbCommand::UpdateSchedule { schedule, reply })
            .await
    }

    pub async fn delete_schedule(&self, id: i64) -> Result<(), DbError> {
        self.send_cmd(|reply| DbCommand::DeleteSchedule { id, reply })
            .await
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Blocked Websites (profile-specific)
    // ─────────────────────────────────────────────────────────────────────────

    pub async fn create_blocked_website(
        &self,
        website: BlockedWebsite,
    ) -> Result<BlockedWebsite, DbError> {
        self.send_cmd(|reply| DbCommand::CreateBlockedWebsite { website, reply })
            .await
    }

    pub async fn list_blocked_websites(
        &self,
        profile_id: String,
    ) -> Result<Vec<BlockedWebsite>, DbError> {
        self.send_cmd(|reply| DbCommand::ListBlockedWebsites { profile_id, reply })
            .await
    }

    pub async fn delete_blocked_website(&self, id: i64) -> Result<(), DbError> {
        self.send_cmd(|reply| DbCommand::DeleteBlockedWebsite { id, reply })
            .await
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Blocked Apps
    // ─────────────────────────────────────────────────────────────────────────

    pub async fn create_blocked_app(&self, app: BlockedApp) -> Result<BlockedApp, DbError> {
        self.send_cmd(|reply| DbCommand::CreateBlockedApp { app, reply })
            .await
    }

    pub async fn get_blocked_app(&self, id: i64) -> Result<Option<BlockedApp>, DbError> {
        self.send_cmd(|reply| DbCommand::GetBlockedApp { id, reply })
            .await
    }

    pub async fn list_blocked_apps(&self, profile_id: String) -> Result<Vec<BlockedApp>, DbError> {
        self.send_cmd(|reply| DbCommand::ListBlockedApps { profile_id, reply })
            .await
    }

    pub async fn delete_blocked_app(&self, id: i64) -> Result<(), DbError> {
        self.send_cmd(|reply| DbCommand::DeleteBlockedApp { id, reply })
            .await
    }

    // ─────────────────────────────────────────────────────────────────────────
    // DOM Rules
    // ─────────────────────────────────────────────────────────────────────────

    pub async fn create_dom_rule(&self, rule: DomRule) -> Result<DomRule, DbError> {
        self.send_cmd(|reply| DbCommand::CreateDomRule { rule, reply })
            .await
    }

    pub async fn get_dom_rule(&self, id: i64) -> Result<Option<DomRule>, DbError> {
        self.send_cmd(|reply| DbCommand::GetDomRule { id, reply })
            .await
    }

    pub async fn list_dom_rules(&self, profile_id: String) -> Result<Vec<DomRule>, DbError> {
        self.send_cmd(|reply| DbCommand::ListDomRules { profile_id, reply })
            .await
    }

    pub async fn delete_dom_rule(&self, id: i64) -> Result<(), DbError> {
        self.send_cmd(|reply| DbCommand::DeleteDomRule { id, reply })
            .await
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Manual Sessions
    // ─────────────────────────────────────────────────────────────────────────

    pub async fn create_manual_session(
        &self,
        session: ManualSession,
    ) -> Result<ManualSession, DbError> {
        self.send_cmd(|reply| DbCommand::CreateManualSession { session, reply })
            .await
    }

    pub async fn get_manual_session(&self, id: i64) -> Result<Option<ManualSession>, DbError> {
        self.send_cmd(|reply| DbCommand::GetManualSession { id, reply })
            .await
    }

    pub async fn get_active_manual_session(&self) -> Result<Option<ManualSession>, DbError> {
        self.send_cmd(|reply| DbCommand::GetActiveManualSession { reply })
            .await
    }

    pub async fn list_manual_sessions(&self) -> Result<Vec<ManualSession>, DbError> {
        self.send_cmd(|reply| DbCommand::ListManualSessions { reply })
            .await
    }

    pub async fn update_manual_session(&self, session: ManualSession) -> Result<(), DbError> {
        self.send_cmd(|reply| DbCommand::UpdateManualSession { session, reply })
            .await
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Global Blocked Websites (profile-independent)
    // ─────────────────────────────────────────────────────────────────────────

    pub async fn add_global_website(&self, domain: String) -> Result<bool, DbError> {
        self.send_cmd(|reply| DbCommand::AddGlobalWebsite { domain, reply })
            .await
    }

    pub async fn remove_global_website(&self, domain: String) -> Result<bool, DbError> {
        self.send_cmd(|reply| DbCommand::RemoveGlobalWebsite { domain, reply })
            .await
    }

    pub async fn list_global_websites(&self) -> Result<Vec<String>, DbError> {
        self.send_cmd(|reply| DbCommand::ListGlobalWebsites { reply })
            .await
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Active Profiles & Policy
    // ─────────────────────────────────────────────────────────────────────────

    /// Get all profiles that are active at the given day/time.
    /// `current_day` should be lowercase: "mon", "tue", "wed", etc.
    /// `current_time` should be "HH:MM" format: "09:30", "14:00", etc.
    pub async fn get_active_profiles(
        &self,
        current_day: String,
        current_time: String,
    ) -> Result<Vec<Profile>, DbError> {
        self.send_cmd(|reply| DbCommand::GetActiveProfiles {
            current_day,
            current_time,
            reply,
        })
        .await
    }

    /// Get the merged policy from all active profiles.
    pub async fn get_active_policy(
        &self,
        current_day: String,
        current_time: String,
    ) -> Result<ActivePolicy, DbError> {
        self.send_cmd(|reply| DbCommand::GetActivePolicy {
            current_day,
            current_time,
            reply,
        })
        .await
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Background thread
// ─────────────────────────────────────────────────────────────────────────────

fn db_thread(conn: Connection, mut rx: mpsc::Receiver<DbCommand>) {
    while let Some(cmd) = rx.blocking_recv() {
        match cmd {
            // ─────────────────────────────────────────────────────────────────
            // Profiles
            // ─────────────────────────────────────────────────────────────────

            DbCommand::CreateProfile { name, reply } => {
                let result = (|| -> Result<Profile, DbError> {
                    let profile = Profile {
                        id: Uuid::new_v4().to_string(),
                        name,
                        enabled: true,
                    };
                    conn.execute(
                        "INSERT INTO profiles (id, name, enabled) VALUES (?1, ?2, ?3)",
                        params![&profile.id, &profile.name, profile.enabled],
                    )?;
                    Ok(profile)
                })();
                let _ = reply.send(result);
            }

            DbCommand::GetProfile { id, reply } => {
                let result = conn
                    .query_row(
                        "SELECT id, name, enabled FROM profiles WHERE id = ?1",
                        params![&id],
                        |row| {
                            Ok(Profile {
                                id: row.get(0)?,
                                name: row.get(1)?,
                                enabled: row.get(2)?,
                            })
                        },
                    )
                    .optional()
                    .map_err(DbError::from);
                let _ = reply.send(result);
            }

            DbCommand::ListProfiles { reply } => {
                let result = (|| -> Result<Vec<Profile>, DbError> {
                    let mut stmt = conn.prepare("SELECT id, name, enabled FROM profiles")?;
                    let profiles = stmt
                        .query_map([], |row| {
                            Ok(Profile {
                                id: row.get(0)?,
                                name: row.get(1)?,
                                enabled: row.get(2)?,
                            })
                        })?
                        .collect::<Result<Vec<_>, _>>()?;
                    Ok(profiles)
                })();
                let _ = reply.send(result);
            }

            DbCommand::UpdateProfile { profile, reply } => {
                let result = conn
                    .execute(
                        "UPDATE profiles SET name = ?1, enabled = ?2 WHERE id = ?3",
                        params![&profile.name, profile.enabled, &profile.id],
                    )
                    .map(|_| ())
                    .map_err(DbError::from);
                let _ = reply.send(result);
            }

            DbCommand::DeleteProfile { id, reply } => {
                let result = conn
                    .execute("DELETE FROM profiles WHERE id = ?1", params![&id])
                    .map(|_| ())
                    .map_err(DbError::from);
                let _ = reply.send(result);
            }

            // ─────────────────────────────────────────────────────────────────
            // Schedules
            // ─────────────────────────────────────────────────────────────────

            DbCommand::CreateSchedule { mut schedule, reply } => {
                let result = (|| -> Result<Schedule, DbError> {
                    let days_json = schedule
                        .days
                        .as_ref()
                        .map(|d| serde_json::to_string(d))
                        .transpose()?;
                    let type_str = schedule
                        .schedule_type
                        .as_ref()
                        .map(|t| match t {
                            ScheduleType::Recurring => "recurring",
                            ScheduleType::OneTime => "one_time",
                        });

                    conn.execute(
                        "INSERT INTO schedules (profile_id, type, days, start_time, end_time, timezone, one_time_date) 
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                        params![
                            schedule.profile_id,
                            type_str,
                            days_json,
                            schedule.start_time,
                            schedule.end_time,
                            schedule.timezone,
                            schedule.one_time_date,
                        ],
                    )?;
                    schedule.id = conn.last_insert_rowid();
                    Ok(schedule)
                })();
                let _ = reply.send(result);
            }

            DbCommand::GetSchedule { id, reply } => {
                let result = (|| -> Result<Option<Schedule>, DbError> {
                    let row = conn
                        .query_row(
                            "SELECT id, profile_id, type, days, start_time, end_time, timezone, one_time_date 
                             FROM schedules WHERE id = ?1",
                            params![id],
                            |row| {
                                Ok((
                                    row.get::<_, i64>(0)?,
                                    row.get::<_, String>(1)?,
                                    row.get::<_, Option<String>>(2)?,
                                    row.get::<_, Option<String>>(3)?,
                                    row.get::<_, Option<String>>(4)?,
                                    row.get::<_, Option<String>>(5)?,
                                    row.get::<_, Option<String>>(6)?,
                                    row.get::<_, Option<String>>(7)?,
                                ))
                            },
                        )
                        .optional()?;

                    match row {
                        Some((id, profile_id, type_str, days_str, start, end, tz, one_time)) => {
                            let schedule_type = type_str.map(|s| match s.as_str() {
                                "recurring" => ScheduleType::Recurring,
                                "one_time" => ScheduleType::OneTime,
                                _ => ScheduleType::Recurring,
                            });
                            let days: Option<Vec<String>> = days_str
                                .map(|s| serde_json::from_str(&s))
                                .transpose()?;

                            Ok(Some(Schedule {
                                id,
                                profile_id,
                                schedule_type,
                                days,
                                start_time: start,
                                end_time: end,
                                timezone: tz,
                                one_time_date: one_time,
                            }))
                        }
                        None => Ok(None),
                    }
                })();
                let _ = reply.send(result);
            }

            DbCommand::ListSchedules { profile_id, reply } => {
                let result = (|| -> Result<Vec<Schedule>, DbError> {
                    let mut stmt = conn.prepare(
                        "SELECT id, profile_id, type, days, start_time, end_time, timezone, one_time_date 
                         FROM schedules WHERE profile_id = ?1",
                    )?;

                    let rows: Vec<(i64, String, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>)> = stmt
                        .query_map(params![profile_id], |row| {
                            Ok((
                                row.get(0)?,
                                row.get(1)?,
                                row.get(2)?,
                                row.get(3)?,
                                row.get(4)?,
                                row.get(5)?,
                                row.get(6)?,
                                row.get(7)?,
                            ))
                        })?
                        .collect::<Result<Vec<_>, _>>()?;

                    let mut schedules = Vec::new();
                    for (id, profile_id, type_str, days_str, start, end, tz, one_time) in rows {
                        let schedule_type = type_str.map(|s| match s.as_str() {
                            "recurring" => ScheduleType::Recurring,
                            "one_time" => ScheduleType::OneTime,
                            _ => ScheduleType::Recurring,
                        });
                        let days: Option<Vec<String>> = days_str
                            .map(|s| serde_json::from_str(&s))
                            .transpose()?;

                        schedules.push(Schedule {
                            id,
                            profile_id,
                            schedule_type,
                            days,
                            start_time: start,
                            end_time: end,
                            timezone: tz,
                            one_time_date: one_time,
                        });
                    }
                    Ok(schedules)
                })();
                let _ = reply.send(result);
            }

            DbCommand::UpdateSchedule { schedule, reply } => {
                let result = (|| -> Result<(), DbError> {
                    let days_json = schedule
                        .days
                        .as_ref()
                        .map(|d| serde_json::to_string(d))
                        .transpose()?;
                    let type_str = schedule
                        .schedule_type
                        .as_ref()
                        .map(|t| match t {
                            ScheduleType::Recurring => "recurring",
                            ScheduleType::OneTime => "one_time",
                        });

                    conn.execute(
                        "UPDATE schedules SET type = ?1, days = ?2, start_time = ?3, end_time = ?4, timezone = ?5, one_time_date = ?6 
                         WHERE id = ?7",
                        params![
                            type_str,
                            days_json,
                            schedule.start_time,
                            schedule.end_time,
                            schedule.timezone,
                            schedule.one_time_date,
                            schedule.id,
                        ],
                    )?;
                    Ok(())
                })();
                let _ = reply.send(result);
            }

            DbCommand::DeleteSchedule { id, reply } => {
                let result = conn
                    .execute("DELETE FROM schedules WHERE id = ?1", params![id])
                    .map(|_| ())
                    .map_err(DbError::from);
                let _ = reply.send(result);
            }

            // ─────────────────────────────────────────────────────────────────
            // Blocked Websites (profile-specific)
            // ─────────────────────────────────────────────────────────────────

            DbCommand::CreateBlockedWebsite { mut website, reply } => {
                let result = (|| -> Result<BlockedWebsite, DbError> {
                    conn.execute(
                        "INSERT INTO blocked_websites (profile_id, domain) VALUES (?1, ?2)",
                        params![website.profile_id, website.domain],
                    )?;
                    website.id = conn.last_insert_rowid();
                    Ok(website)
                })();
                let _ = reply.send(result);
            }

            DbCommand::ListBlockedWebsites { profile_id, reply } => {
                let result = (|| -> Result<Vec<BlockedWebsite>, DbError> {
                    let mut stmt = conn.prepare(
                        "SELECT id, profile_id, domain FROM blocked_websites WHERE profile_id = ?1",
                    )?;
                    let websites = stmt
                        .query_map(params![profile_id], |row| {
                            Ok(BlockedWebsite {
                                id: row.get(0)?,
                                profile_id: row.get(1)?,
                                domain: row.get(2)?,
                            })
                        })?
                        .collect::<Result<Vec<_>, _>>()?;
                    Ok(websites)
                })();
                let _ = reply.send(result);
            }

            DbCommand::DeleteBlockedWebsite { id, reply } => {
                let result = conn
                    .execute("DELETE FROM blocked_websites WHERE id = ?1", params![id])
                    .map(|_| ())
                    .map_err(DbError::from);
                let _ = reply.send(result);
            }

            // ─────────────────────────────────────────────────────────────────
            // Blocked Apps
            // ─────────────────────────────────────────────────────────────────

            DbCommand::CreateBlockedApp { mut app, reply } => {
                let result = (|| -> Result<BlockedApp, DbError> {
                    conn.execute(
                        "INSERT INTO blocked_apps (profile_id, app_identifier) VALUES (?1, ?2)",
                        params![app.profile_id, app.app_identifier],
                    )?;
                    app.id = conn.last_insert_rowid();
                    Ok(app)
                })();
                let _ = reply.send(result);
            }

            DbCommand::GetBlockedApp { id, reply } => {
                let result = conn
                    .query_row(
                        "SELECT id, profile_id, app_identifier FROM blocked_apps WHERE id = ?1",
                        params![id],
                        |row| {
                            Ok(BlockedApp {
                                id: row.get(0)?,
                                profile_id: row.get(1)?,
                                app_identifier: row.get(2)?,
                            })
                        },
                    )
                    .optional()
                    .map_err(DbError::from);
                let _ = reply.send(result);
            }

            DbCommand::ListBlockedApps { profile_id, reply } => {
                let result = (|| -> Result<Vec<BlockedApp>, DbError> {
                    let mut stmt = conn.prepare(
                        "SELECT id, profile_id, app_identifier FROM blocked_apps WHERE profile_id = ?1",
                    )?;
                    let apps = stmt
                        .query_map(params![profile_id], |row| {
                            Ok(BlockedApp {
                                id: row.get(0)?,
                                profile_id: row.get(1)?,
                                app_identifier: row.get(2)?,
                            })
                        })?
                        .collect::<Result<Vec<_>, _>>()?;
                    Ok(apps)
                })();
                let _ = reply.send(result);
            }

            DbCommand::DeleteBlockedApp { id, reply } => {
                let result = conn
                    .execute("DELETE FROM blocked_apps WHERE id = ?1", params![id])
                    .map(|_| ())
                    .map_err(DbError::from);
                let _ = reply.send(result);
            }

            // ─────────────────────────────────────────────────────────────────
            // DOM Rules
            // ─────────────────────────────────────────────────────────────────

            DbCommand::CreateDomRule { mut rule, reply } => {
                let result = (|| -> Result<DomRule, DbError> {
                    conn.execute(
                        "INSERT INTO dom_rules (profile_id, site, toggle) VALUES (?1, ?2, ?3)",
                        params![rule.profile_id, rule.site, rule.toggle],
                    )?;
                    rule.id = conn.last_insert_rowid();
                    Ok(rule)
                })();
                let _ = reply.send(result);
            }

            DbCommand::GetDomRule { id, reply } => {
                let result = conn
                    .query_row(
                        "SELECT id, profile_id, site, toggle FROM dom_rules WHERE id = ?1",
                        params![id],
                        |row| {
                            Ok(DomRule {
                                id: row.get(0)?,
                                profile_id: row.get(1)?,
                                site: row.get(2)?,
                                toggle: row.get(3)?,
                            })
                        },
                    )
                    .optional()
                    .map_err(DbError::from);
                let _ = reply.send(result);
            }

            DbCommand::ListDomRules { profile_id, reply } => {
                let result = (|| -> Result<Vec<DomRule>, DbError> {
                    let mut stmt = conn.prepare(
                        "SELECT id, profile_id, site, toggle FROM dom_rules WHERE profile_id = ?1",
                    )?;
                    let rules = stmt
                        .query_map(params![profile_id], |row| {
                            Ok(DomRule {
                                id: row.get(0)?,
                                profile_id: row.get(1)?,
                                site: row.get(2)?,
                                toggle: row.get(3)?,
                            })
                        })?
                        .collect::<Result<Vec<_>, _>>()?;
                    Ok(rules)
                })();
                let _ = reply.send(result);
            }

            DbCommand::DeleteDomRule { id, reply } => {
                let result = conn
                    .execute("DELETE FROM dom_rules WHERE id = ?1", params![id])
                    .map(|_| ())
                    .map_err(DbError::from);
                let _ = reply.send(result);
            }

            // ─────────────────────────────────────────────────────────────────
            // Manual Sessions
            // ─────────────────────────────────────────────────────────────────

            DbCommand::CreateManualSession { mut session, reply } => {
                let result = (|| -> Result<ManualSession, DbError> {
                    conn.execute(
                        "INSERT INTO manual_sessions (started_at, duration_secs, ended_at, policy_json) 
                         VALUES (?1, ?2, ?3, ?4)",
                        params![
                            session.started_at,
                            session.duration_secs,
                            session.ended_at,
                            session.policy_json,
                        ],
                    )?;
                    session.id = conn.last_insert_rowid();
                    Ok(session)
                })();
                let _ = reply.send(result);
            }

            DbCommand::GetManualSession { id, reply } => {
                let result = conn
                    .query_row(
                        "SELECT id, started_at, duration_secs, ended_at, policy_json 
                         FROM manual_sessions WHERE id = ?1",
                        params![id],
                        |row| {
                            Ok(ManualSession {
                                id: row.get(0)?,
                                started_at: row.get(1)?,
                                duration_secs: row.get(2)?,
                                ended_at: row.get(3)?,
                                policy_json: row.get(4)?,
                            })
                        },
                    )
                    .optional()
                    .map_err(DbError::from);
                let _ = reply.send(result);
            }

            DbCommand::GetActiveManualSession { reply } => {
                let result = conn
                    .query_row(
                        "SELECT id, started_at, duration_secs, ended_at, policy_json 
                         FROM manual_sessions 
                         WHERE ended_at IS NULL 
                         ORDER BY id DESC LIMIT 1",
                        [],
                        |row| {
                            Ok(ManualSession {
                                id: row.get(0)?,
                                started_at: row.get(1)?,
                                duration_secs: row.get(2)?,
                                ended_at: row.get(3)?,
                                policy_json: row.get(4)?,
                            })
                        },
                    )
                    .optional()
                    .map_err(DbError::from);
                let _ = reply.send(result);
            }

            DbCommand::ListManualSessions { reply } => {
                let result = (|| -> Result<Vec<ManualSession>, DbError> {
                    let mut stmt = conn.prepare(
                        "SELECT id, started_at, duration_secs, ended_at, policy_json FROM manual_sessions",
                    )?;
                    let sessions = stmt
                        .query_map([], |row| {
                            Ok(ManualSession {
                                id: row.get(0)?,
                                started_at: row.get(1)?,
                                duration_secs: row.get(2)?,
                                ended_at: row.get(3)?,
                                policy_json: row.get(4)?,
                            })
                        })?
                        .collect::<Result<Vec<_>, _>>()?;
                    Ok(sessions)
                })();
                let _ = reply.send(result);
            }

            DbCommand::UpdateManualSession { session, reply } => {
                let result = conn
                    .execute(
                        "UPDATE manual_sessions SET ended_at = ?1, policy_json = ?2 WHERE id = ?3",
                        params![session.ended_at, session.policy_json, session.id],
                    )
                    .map(|_| ())
                    .map_err(DbError::from);
                let _ = reply.send(result);
            }

            // ─────────────────────────────────────────────────────────────────
            // Global Blocked Websites
            // ─────────────────────────────────────────────────────────────────

            DbCommand::AddGlobalWebsite { domain, reply } => {
                let result = conn
                    .execute(
                        "INSERT OR IGNORE INTO global_blocked_websites (domain) VALUES (?1)",
                        params![&domain],
                    )
                    .map(|rows| rows > 0)
                    .map_err(DbError::from);
                let _ = reply.send(result);
            }

            DbCommand::RemoveGlobalWebsite { domain, reply } => {
                let result = conn
                    .execute(
                        "DELETE FROM global_blocked_websites WHERE domain = ?1",
                        params![&domain],
                    )
                    .map(|rows| rows > 0)
                    .map_err(DbError::from);
                let _ = reply.send(result);
            }

            DbCommand::ListGlobalWebsites { reply } => {
                let result = (|| -> Result<Vec<String>, DbError> {
                    let mut stmt = conn
                        .prepare("SELECT domain FROM global_blocked_websites ORDER BY domain")?;
                    let websites = stmt
                        .query_map([], |row| row.get(0))?
                        .collect::<Result<Vec<String>, _>>()?;
                    Ok(websites)
                })();
                let _ = reply.send(result);
            }

            // ─────────────────────────────────────────────────────────────────
            // Active Profiles & Policy
            // ─────────────────────────────────────────────────────────────────

            DbCommand::GetActiveProfiles {
                current_day,
                current_time,
                reply,
            } => {
                let result = (|| -> Result<Vec<Profile>, DbError> {
                    let pattern = format!("%\"{}%", current_day);
                    let mut stmt = conn.prepare(
                        "SELECT DISTINCT p.id, p.name, p.enabled 
                         FROM profiles p
                         JOIN schedules s ON s.profile_id = p.id
                         WHERE p.enabled = 1
                           AND s.days LIKE ?1
                           AND s.start_time <= ?2
                           AND s.end_time > ?2",
                    )?;
                    let profiles = stmt
                        .query_map(params![pattern, &current_time], |row| {
                            Ok(Profile {
                                id: row.get(0)?,
                                name: row.get(1)?,
                                enabled: row.get(2)?,
                            })
                        })?
                        .collect::<Result<Vec<_>, _>>()?;
                    Ok(profiles)
                })();
                let _ = reply.send(result);
            }

            DbCommand::GetActivePolicy {
                current_day,
                current_time,
                reply,
            } => {
                let result = (|| -> Result<ActivePolicy, DbError> {
                    let mut policy = ActivePolicy::default();

                    // Get active profiles
                    let pattern = format!("%\"{}%", current_day);
                    let mut profile_stmt = conn.prepare(
                        "SELECT DISTINCT p.id
                         FROM profiles p
                         JOIN schedules s ON s.profile_id = p.id
                         WHERE p.enabled = 1
                           AND s.days LIKE ?1
                           AND s.start_time <= ?2
                           AND s.end_time > ?2",
                    )?;
                    let profile_ids: Vec<String> = profile_stmt
                        .query_map(params![pattern, &current_time], |row| row.get(0))?
                        .collect::<Result<Vec<_>, _>>()?;

                    // Collect blocked websites from active profiles
                    for profile_id in &profile_ids {
                        let mut stmt = conn.prepare(
                            "SELECT domain FROM blocked_websites WHERE profile_id = ?1",
                        )?;
                        let domains: Vec<String> = stmt
                            .query_map(params![profile_id], |row| row.get(0))?
                            .collect::<Result<Vec<_>, _>>()?;
                        policy.blocked_websites.extend(domains);
                    }

                    // Collect blocked apps from active profiles
                    for profile_id in &profile_ids {
                        let mut stmt = conn.prepare(
                            "SELECT app_identifier FROM blocked_apps WHERE profile_id = ?1",
                        )?;
                        let apps: Vec<String> = stmt
                            .query_map(params![profile_id], |row| row.get(0))?
                            .collect::<Result<Vec<_>, _>>()?;
                        policy.blocked_apps.extend(apps);
                    }

                    // Collect DOM rules from active profiles
                    for profile_id in &profile_ids {
                        let mut stmt = conn.prepare(
                            "SELECT id, profile_id, site, toggle FROM dom_rules WHERE profile_id = ?1",
                        )?;
                        let rules: Vec<DomRule> = stmt
                            .query_map(params![profile_id], |row| {
                                Ok(DomRule {
                                    id: row.get(0)?,
                                    profile_id: row.get(1)?,
                                    site: row.get(2)?,
                                    toggle: row.get(3)?,
                                })
                            })?
                            .collect::<Result<Vec<_>, _>>()?;
                        policy.dom_rules.extend(rules);
                    }

                    // Add global blocked websites
                    let mut global_stmt =
                        conn.prepare("SELECT domain FROM global_blocked_websites")?;
                    let global_domains: Vec<String> = global_stmt
                        .query_map([], |row| row.get(0))?
                        .collect::<Result<Vec<_>, _>>()?;
                    policy.blocked_websites.extend(global_domains);

                    // Deduplicate
                    policy.blocked_websites.sort();
                    policy.blocked_websites.dedup();
                    policy.blocked_apps.sort();
                    policy.blocked_apps.dedup();

                    Ok(policy)
                })();
                let _ = reply.send(result);
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn test_db() -> DbHandle {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.db");
        let path = Box::leak(Box::new(path));
        DbHandle::open(path).unwrap()
    }

    #[tokio::test]
    async fn open_creates_tables() {
        let db = test_db();
        let profiles = db.list_profiles().await.unwrap();
        assert!(profiles.is_empty());
    }

    #[tokio::test]
    async fn create_and_get_profile() {
        let db = test_db();
        let profile = db.create_profile("Work Focus".into()).await.unwrap();
        assert_eq!(profile.name, "Work Focus");
        assert!(profile.enabled);

        let fetched = db.get_profile(profile.id.clone()).await.unwrap().unwrap();
        assert_eq!(fetched.name, "Work Focus");
    }

    #[tokio::test]
    async fn delete_profile_cascades() {
        let db = test_db();
        let profile = db.create_profile("Test".into()).await.unwrap();

        // Add a schedule
        let schedule = Schedule {
            id: 0,
            profile_id: profile.id.clone(),
            schedule_type: Some(ScheduleType::Recurring),
            days: Some(vec!["mon".into(), "tue".into()]),
            start_time: Some("09:00".into()),
            end_time: Some("17:00".into()),
            timezone: Some("America/New_York".into()),
            one_time_date: None,
        };
        db.create_schedule(schedule).await.unwrap();

        // Delete profile
        db.delete_profile(profile.id.clone()).await.unwrap();

        // Schedule should be gone too (CASCADE)
        let schedules = db.list_schedules(profile.id).await.unwrap();
        assert!(schedules.is_empty());
    }

    #[tokio::test]
    async fn global_websites() {
        let db = test_db();
        
        assert!(db.add_global_website("reddit.com".into()).await.unwrap());
        assert!(db.add_global_website("youtube.com".into()).await.unwrap());
        
        // Duplicate returns false
        assert!(!db.add_global_website("reddit.com".into()).await.unwrap());

        let sites = db.list_global_websites().await.unwrap();
        assert_eq!(sites, vec!["reddit.com", "youtube.com"]);

        assert!(db.remove_global_website("reddit.com".into()).await.unwrap());
        assert!(!db.remove_global_website("reddit.com".into()).await.unwrap());
    }

    #[tokio::test]
    async fn active_policy() {
        let db = test_db();

        // Create profile
        let profile = db.create_profile("Work".into()).await.unwrap();

        // Add schedule for Monday 9-17
        let schedule = Schedule {
            id: 0,
            profile_id: profile.id.clone(),
            schedule_type: Some(ScheduleType::Recurring),
            days: Some(vec!["mon".into()]),
            start_time: Some("09:00".into()),
            end_time: Some("17:00".into()),
            timezone: Some("America/New_York".into()),
            one_time_date: None,
        };
        db.create_schedule(schedule).await.unwrap();

        // Add blocked website
        db.create_blocked_website(BlockedWebsite {
            id: 0,
            profile_id: profile.id.clone(),
            domain: "twitter.com".into(),
        })
        .await
        .unwrap();

        // Add global website
        db.add_global_website("reddit.com".into()).await.unwrap();

        // Query at Monday 10:00 - should be active
        let policy = db
            .get_active_policy("mon".into(), "10:00".into())
            .await
            .unwrap();
        assert!(policy.blocked_websites.contains(&"twitter.com".to_string()));
        assert!(policy.blocked_websites.contains(&"reddit.com".to_string()));

        // Query at Tuesday 10:00 - profile not active, but global still is
        let policy = db
            .get_active_policy("tue".into(), "10:00".into())
            .await
            .unwrap();
        assert!(!policy.blocked_websites.contains(&"twitter.com".to_string()));
        assert!(policy.blocked_websites.contains(&"reddit.com".to_string()));
    }
}
