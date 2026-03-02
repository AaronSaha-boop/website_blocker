// src/hooks/useDaemon.ts
//
// React hooks and functions for calling Tauri commands.
// This bridges your React UI to the Rust daemon.

import { invoke } from "@tauri-apps/api/tauri";

// ─────────────────────────────────────────────────────────────────────────────
// Types (matching your Rust DaemonMessage variants)
// ─────────────────────────────────────────────────────────────────────────────

export interface Profile {
  id: string;
  name: string;
  enabled: boolean;
}

export interface Schedule {
  id: number;
  profile_id: string;
  schedule_type: string | null;
  days: string[] | null;
  start_time: string | null;
  end_time: string | null;
  timezone: string | null;
  one_time_date: string | null;
}

export interface BlockedWebsite {
  id: number;
  profile_id: string;
  domain: string;
}

export interface BlockedApp {
  id: number;
  profile_id: string;
  app_identifier: string;
}

export interface DomRule {
  id: number;
  profile_id: string;
  site: string;
  toggle: string;
}

export interface ActivePolicy {
  blocked_websites: string[];
  blocked_apps: string[];
  dom_rules: { site: string; toggle: string }[];
}

// DaemonMessage is a tagged union in Rust - we represent it as discriminated union
export type DaemonMessage =
  | { Started: { duration: number } }
  | { Stopped: null }
  | { StatusIdle: { sites: string[] } }
  | { StatusWithTime: { time_left: number; sites: string[] } }
  | { Pong: null }
  | { Error: string }
  | { Profile: Profile }
  | { ProfileList: Profile[] }
  | { ProfileUpdated: null }
  | { ProfileDeleted: null }
  | { Schedule: Schedule }
  | { ScheduleList: Schedule[] }
  | { ScheduleUpdated: null }
  | { ScheduleDeleted: null }
  | { BlockedWebsite: BlockedWebsite }
  | { BlockedWebsiteList: BlockedWebsite[] }
  | { BlockedWebsiteDeleted: null }
  | { BlockedApp: BlockedApp }
  | { BlockedAppList: BlockedApp[] }
  | { BlockedAppDeleted: null }
  | { DomRule: DomRule }
  | { DomRuleList: DomRule[] }
  | { DomRuleDeleted: null }
  | { GlobalWebsiteAdded: boolean }
  | { GlobalWebsiteRemoved: boolean }
  | { GlobalWebsiteList: string[] }
  | { ActivePolicy: ActivePolicy };

// ─────────────────────────────────────────────────────────────────────────────
// API Functions
// ─────────────────────────────────────────────────────────────────────────────

export const daemon = {
  // Session
  async getStatus(): Promise<DaemonMessage> {
    return invoke("get_status");
  },

  async startSession(duration: number): Promise<DaemonMessage> {
    return invoke("start_session", { duration });
  },

  async stopSession(): Promise<DaemonMessage> {
    return invoke("stop_session");
  },

  // Global websites
  async listGlobalWebsites(): Promise<DaemonMessage> {
    return invoke("list_global_websites");
  },

  async addGlobalWebsite(domain: string): Promise<DaemonMessage> {
    return invoke("add_global_website", { domain });
  },

  async removeGlobalWebsite(domain: string): Promise<DaemonMessage> {
    return invoke("remove_global_website", { domain });
  },

  // Profiles
  async createProfile(name: string): Promise<DaemonMessage> {
    return invoke("create_profile", { name });
  },

  async listProfiles(): Promise<DaemonMessage> {
    return invoke("list_profiles");
  },

  async updateProfile(id: string, name: string, enabled: boolean): Promise<DaemonMessage> {
    return invoke("update_profile", { id, name, enabled });
  },

  async deleteProfile(id: string): Promise<DaemonMessage> {
    return invoke("delete_profile", { id });
  },

  // Schedules
  async listSchedules(profileId: string): Promise<DaemonMessage> {
    return invoke("list_schedules", { profileId });
  },

  async createSchedule(
    profileId: string,
    days: string[],
    startTime: string,
    endTime: string
  ): Promise<DaemonMessage> {
    return invoke("create_schedule", {
      profileId,
      days,
      startTime,
      endTime,
    });
  },

  async deleteSchedule(id: number): Promise<DaemonMessage> {
    return invoke("delete_schedule", { id });
  },

  // Blocked websites (per profile)
  async listBlockedWebsites(profileId: string): Promise<DaemonMessage> {
    return invoke("list_blocked_websites", { profileId });
  },

  async addBlockedWebsite(profileId: string, domain: string): Promise<DaemonMessage> {
    return invoke("add_blocked_website", { profileId, domain });
  },

  async deleteBlockedWebsite(id: number): Promise<DaemonMessage> {
    return invoke("delete_blocked_website", { id });
  },

  // Blocked apps (per profile)
  async listBlockedApps(profileId: string): Promise<DaemonMessage> {
    return invoke("list_blocked_apps", { profileId });
  },

  async addBlockedApp(profileId: string, appIdentifier: string): Promise<DaemonMessage> {
    return invoke("add_blocked_app", { profileId, appIdentifier });
  },

  async deleteBlockedApp(id: number): Promise<DaemonMessage> {
    return invoke("delete_blocked_app", { id });
  },

  // DOM rules (per profile)
  async listDomRules(profileId: string): Promise<DaemonMessage> {
    return invoke("list_dom_rules", { profileId });
  },

  async addDomRule(profileId: string, site: string, toggle: string): Promise<DaemonMessage> {
    return invoke("add_dom_rule", { profileId, site, toggle });
  },

  async deleteDomRule(id: number): Promise<DaemonMessage> {
    return invoke("delete_dom_rule", { id });
  },

  // Active policy
  async getActivePolicy(): Promise<DaemonMessage> {
    return invoke("get_active_policy");
  },
};

// ─────────────────────────────────────────────────────────────────────────────
// Helper to extract data from DaemonMessage
// ─────────────────────────────────────────────────────────────────────────────

export function isError(msg: DaemonMessage): msg is { Error: string } {
  return "Error" in msg;
}

export function getError(msg: DaemonMessage): string | null {
  if ("Error" in msg) return msg.Error;
  return null;
}
