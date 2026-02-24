// src/main.rs

use clap::Parser;
use std::time::Duration;

use dark_pattern_blocker::db::{BlockedApp, BlockedWebsite, DomRule, Schedule};
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

    /// Add a website to the global block list
    AddGlobal { domain: String },

    /// Remove a website from the global block list
    RemoveGlobal { domain: String },

    /// List all globally blocked websites
    ListGlobal,

    /// Create a new profile
    CreateProfile { name: String },

    /// List all profiles
    ListProfiles,

    /// Delete a profile
    DeleteProfile { id: String },

    /// List all schedules for a profile
    ListSchedules { profile_id: String },

    /// Delete a schedule
    DeleteSchedule { id: i64 },

    /// Add a website to a profile's blocklist
    AddBlockedWebsite { profile_id: String, domain: String },

    /// List all blocked websites for a profile
    ListBlockedWebsites { profile_id: String },

    /// Remove a website from a profile's blocklist
    DeleteBlockedWebsite { id: i64 },

    /// Add an app to a profile's blocklist
    AddBlockedApp { profile_id: String, app_identifier: String },

    /// List all blocked apps for a profile
    ListBlockedApps { profile_id: String },

    /// Remove an app from a profile's blocklist
    DeleteBlockedApp { id: i64 },

    /// Add a DOM rule to a profile
    AddDomRule { profile_id: String, site: String, toggle: String },

    /// List all DOM rules for a profile
    ListDomRules { profile_id: String },

    /// Remove a DOM rule from a profile
    DeleteDomRule { id: i64 },
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

        // Global websites
        Command::AddGlobal { domain } => ClientMessage::AddGlobalWebsite { domain },
        Command::RemoveGlobal { domain } => ClientMessage::RemoveGlobalWebsite { domain },
        Command::ListGlobal => ClientMessage::ListGlobalWebsites,

        // Profiles
        Command::CreateProfile { name } => ClientMessage::CreateProfile { name },
        Command::ListProfiles => ClientMessage::ListProfiles,
        Command::DeleteProfile { id } => ClientMessage::DeleteProfile { id },

        // Schedules
        Command::ListSchedules { profile_id } => ClientMessage::ListSchedules { profile_id },
        Command::DeleteSchedule { id } => ClientMessage::DeleteSchedule { id },

        // Blocked Websites
        Command::AddBlockedWebsite { profile_id, domain } => {
            ClientMessage::CreateBlockedWebsite { website: BlockedWebsite { id: 0, profile_id, domain } }
        }
        Command::ListBlockedWebsites { profile_id } => {
            ClientMessage::ListBlockedWebsites { profile_id }
        }
        Command::DeleteBlockedWebsite { id } => ClientMessage::DeleteBlockedWebsite { id },

        // Blocked Apps
        Command::AddBlockedApp { profile_id, app_identifier } => {
            ClientMessage::CreateBlockedApp { app: BlockedApp { id: 0, profile_id, app_identifier } }
        }
        Command::ListBlockedApps { profile_id } => ClientMessage::ListBlockedApps { profile_id },
        Command::DeleteBlockedApp { id } => ClientMessage::DeleteBlockedApp { id },

        // DOM Rules
        Command::AddDomRule { profile_id, site, toggle } => {
            ClientMessage::CreateDomRule { rule: DomRule { id: 0, profile_id, site, toggle } }
        }
        Command::ListDomRules { profile_id } => ClientMessage::ListDomRules { profile_id },
        Command::DeleteDomRule { id } => ClientMessage::DeleteDomRule { id },

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
        DaemonMessage::StatusIdle { sites } => {
            println!("Idle");
            if !sites.is_empty() {
                println!("Blocked websites:");
                for site in &sites {
                    println!("  {}", site);
                }
            }
        }
        DaemonMessage::StatusWithTime { time_left, sites } => {
            println!("Active: {} remaining", format_duration(time_left));
            if !sites.is_empty() {
                println!("Blocked websites:");
                for site in &sites {
                    println!("  {}", site);
                }
            }
        }
        DaemonMessage::Pong => {
            println!("Pong");
        }
        DaemonMessage::Error(msg) => {
            eprintln!("{}", msg);
            std::process::exit(1);
        }

        // Profiles
        DaemonMessage::Profile(profile) => {
            println!("ID: {}\nName: {}\nEnabled: {}", profile.id, profile.name, profile.enabled);
        }
        DaemonMessage::ProfileList(profiles) => {
            if profiles.is_empty() {
                println!("No profiles");
            } else {
                for profile in &profiles {
                    println!("ID: {}\nName: {}\nEnabled: {}\n", profile.id, profile.name, profile.enabled);
                }
            }
        }
        DaemonMessage::ProfileUpdated => {
            println!("Profile updated");
        }
        DaemonMessage::ProfileDeleted => {
            println!("Profile deleted");
        }

        // Schedules
        DaemonMessage::Schedule(schedule) => {
            println!("ID: {}\nProfile ID: {}\nType: {:?}", schedule.id, schedule.profile_id, schedule.schedule_type);
        }
        DaemonMessage::ScheduleList(schedules) => {
            if schedules.is_empty() {
                println!("No schedules");
            } else {
                for schedule in &schedules {
                    println!("ID: {}\nProfile ID: {}\nType: {:?}\n", schedule.id, schedule.profile_id, schedule.schedule_type);
                }
            }
        }
        DaemonMessage::ScheduleUpdated => {
            println!("Schedule updated");
        }
        DaemonMessage::ScheduleDeleted => {
            println!("Schedule deleted");
        }

        // Blocked Websites
        DaemonMessage::BlockedWebsite(website) => {
            println!("ID: {}\nProfile ID: {}\nDomain: {}", website.id, website.profile_id, website.domain);
        }
        DaemonMessage::BlockedWebsiteList(websites) => {
            if websites.is_empty() {
                println!("No blocked websites");
            } else {
                for website in &websites {
                    println!("ID: {}\nProfile ID: {}\nDomain: {}\n", website.id, website.profile_id, website.domain);
                }
            }
        }
        DaemonMessage::BlockedWebsiteDeleted => {
            println!("Blocked website deleted");
        }

        // Blocked Apps
        DaemonMessage::BlockedApp(app) => {
            println!("ID: {}\nProfile ID: {}\nApp Identifier: {}", app.id, app.profile_id, app.app_identifier);
        }
        DaemonMessage::BlockedAppList(apps) => {
            if apps.is_empty() {
                println!("No blocked apps");
            } else {
                for app in &apps {
                    println!("ID: {}\nProfile ID: {}\nApp Identifier: {}\n", app.id, app.profile_id, app.app_identifier);
                }
            }
        }
        DaemonMessage::BlockedAppDeleted => {
            println!("Blocked app deleted");
        }

        // DOM Rules
        DaemonMessage::DomRule(rule) => {
            println!("ID: {}\nProfile ID: {}\nSite: {}\nToggle: {}", rule.id, rule.profile_id, rule.site, rule.toggle);
        }
        DaemonMessage::DomRuleList(rules) => {
            if rules.is_empty() {
                println!("No DOM rules");
            } else {
                for rule in &rules {
                    println!("ID: {}\nProfile ID: {}\nSite: {}\nToggle: {}\n", rule.id, rule.profile_id, rule.site, rule.toggle);
                }
            }
        }
        DaemonMessage::DomRuleDeleted => {
            println!("DOM rule deleted");
        }

        // Manual Sessions
        DaemonMessage::ManualSession(session) => {
            if let Some(session) = session {
                println!("ID: {}\nStarted At: {}\nDuration: {}", session.id, session.started_at, session.duration_secs);
            } else {
                println!("No active manual session");
            }
        }
        DaemonMessage::ManualSessionList(sessions) => {
            if sessions.is_empty() {
                println!("No manual sessions");
            } else {
                for session in &sessions {
                    println!("ID: {}\nStarted At: {}\nDuration: {}\n", session.id, session.started_at, session.duration_secs);
                }
            }
        }
        DaemonMessage::ManualSessionUpdated => {
            println!("Manual session updated");
        }

        // Global Blocked Websites
        DaemonMessage::GlobalWebsiteAdded(success) => {
            if success {
                println!("Website added to global blocklist");
            } else {
                println!("Website already in global blocklist");
            }
        }
        DaemonMessage::GlobalWebsiteRemoved(success) => {
            if success {
                println!("Website removed from global blocklist");
            } else {
                println!("Website not in global blocklist");
            }
        }
        DaemonMessage::GlobalWebsiteList(websites) => {
            if websites.is_empty() {
                println!("No global blocked websites");
            } else {
                for site in &websites {
                    println!("  {}", site);
                }
            }
        }

        // Active Policy
        DaemonMessage::ActivePolicy(policy) => {
            println!("Active Policy:");
            println!("  Blocked Websites:");
            for site in &policy.blocked_websites {
                println!("    {}", site);
            }
            println!("  Blocked Apps:");
            for app in &policy.blocked_apps {
                println!("    {}", app);
            }
            println!("  DOM Rules:");
            for rule in &policy.dom_rules {
                println!("    {}: {}", rule.site, rule.toggle);
            }
        }
    }
}
