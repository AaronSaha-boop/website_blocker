// src-tauri/src/main.rs
//
// Tauri application entry point.
// Registers all commands and runs the app.

#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

mod commands;

use commands::*;

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            // Session
            get_status,
            start_session,
            stop_session,
            // Global websites
            list_global_websites,
            add_global_website,
            remove_global_website,
            // Profiles
            create_profile,
            update_profile,
            list_profiles,
            delete_profile,
            // Schedules
            list_schedules,
            create_schedule,
            delete_schedule,
            // Blocked websites (per profile)
            list_blocked_websites,
            add_blocked_website,
            delete_blocked_website,
            // Blocked apps (per profile)
            list_blocked_apps,
            add_blocked_app,
            delete_blocked_app,
            // DOM rules (per profile)
            list_dom_rules,
            add_dom_rule,
            delete_dom_rule,
            // Active policy
            get_active_policy,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
