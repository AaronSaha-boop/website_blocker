// src/bin/gui.rs — Tauri entry point for the Dark Pattern Blocker GUI

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use dark_pattern_blocker::commands;

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            // Session
            commands::start_session,
            commands::stop_session,
            commands::get_status,
            // Global websites
            commands::add_global_website,
            commands::remove_global_website,
            commands::list_global_websites,
            // Profiles
            commands::create_profile,
            commands::list_profiles,
            commands::update_profile,
            commands::delete_profile,
            // Schedules
            commands::create_schedule,
            commands::list_schedules,
            commands::delete_schedule,
            // Active policy
            commands::get_active_policy,
            // Blocked websites (per-profile)
            commands::add_blocked_website,
            commands::list_blocked_websites,
            commands::delete_blocked_website,
            // Blocked apps (per-profile)
            commands::add_blocked_app,
            commands::list_blocked_apps,
            commands::delete_blocked_app,
            // DOM rules (per-profile)
            commands::add_dom_rule,
            commands::list_dom_rules,
            commands::delete_dom_rule,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
