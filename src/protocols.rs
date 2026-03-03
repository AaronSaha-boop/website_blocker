// src/protocols.rs

use serde::{Serialize, Deserialize, de::DeserializeOwned};
use thiserror::Error;

use crate::db::{
    ActivePolicy, BlockedApp, BlockedWebsite, DomRule, ManualSession, Profile, Schedule,
};

#[derive(Debug, PartialEq, Error)]
pub enum ProtocolError {
    #[error("Encode error: {0}")]
    EncodeError(String),
    #[error("Decode error: {0}")]
    DecodeError(String),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub enum ClientMessage {
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

    // Blocked Websites (profile-specific)
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

    // Subscriptions
    SubscribePolicyChanges,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub enum DaemonMessage {
    Pong,
    Started { duration: u64 },
    StatusWithTime { time_left: u64, sites: Vec<String> },
    StatusIdle { sites: Vec<String> },
    Stopped,
    Error(String),

    // Profiles
    Profile(Profile),
    ProfileList(Vec<Profile>),
    ProfileUpdated,
    ProfileDeleted,

    // Schedules
    Schedule(Schedule),
    ScheduleList(Vec<Schedule>),
    ScheduleUpdated,
    ScheduleDeleted,

    // Blocked Websites
    BlockedWebsite(BlockedWebsite),
    BlockedWebsiteList(Vec<BlockedWebsite>),
    BlockedWebsiteDeleted,

    // Blocked Apps
    BlockedApp(BlockedApp),
    BlockedAppList(Vec<BlockedApp>),
    BlockedAppDeleted,

    // DOM Rules
    DomRule(DomRule),
    DomRuleList(Vec<DomRule>),
    DomRuleDeleted,

    // Manual Sessions
    ManualSession(Option<ManualSession>),
    ManualSessionList(Vec<ManualSession>),
    ManualSessionUpdated,

    // Global Blocked Websites
    GlobalWebsiteAdded(bool),
    GlobalWebsiteRemoved(bool),
    GlobalWebsiteList(Vec<String>),

    // Active Policy
    ActivePolicy(ActivePolicy),

    // Subscriptions / Push
    Subscribed,
    PolicyChanged(ActivePolicy),
}

pub fn encode<T: Serialize>(msg: &T) -> Result<Vec<u8>, ProtocolError> {
    rmp_serde::to_vec(msg)
        .map_err(|e| ProtocolError::EncodeError(e.to_string()))
}

pub fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, ProtocolError> {
    rmp_serde::from_slice(bytes)
        .map_err(|e| ProtocolError::DecodeError(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip_client(msg: ClientMessage) {
        let bytes = encode(&msg).unwrap();
        let decoded: ClientMessage = decode(&bytes).unwrap();
        assert_eq!(decoded, msg);
    }

    fn roundtrip_daemon(msg: DaemonMessage) {
        let bytes = encode(&msg).unwrap();
        let decoded: DaemonMessage = decode(&bytes).unwrap();
        assert_eq!(decoded, msg);
    }

    #[test]
    fn ping_roundtrip() { roundtrip_client(ClientMessage::Ping); }

    #[test]
    fn start_roundtrip() { roundtrip_client(ClientMessage::Start { duration: 10 }); }

    #[test]
    fn stop_roundtrip() { roundtrip_client(ClientMessage::Stop); }

    #[test]
    fn get_status_roundtrip() { roundtrip_client(ClientMessage::GetStatus); }

    #[test]
    fn add_global_website_roundtrip() {
        roundtrip_client(ClientMessage::AddGlobalWebsite { domain: "reddit.com".into() });
    }

    #[test]
    fn remove_global_website_roundtrip() {
        roundtrip_client(ClientMessage::RemoveGlobalWebsite { domain: "reddit.com".into() });
    }

    #[test]
    fn list_global_websites_roundtrip() { roundtrip_client(ClientMessage::ListGlobalWebsites); }

    #[test]
    fn create_profile_roundtrip() {
        roundtrip_client(ClientMessage::CreateProfile { name: "Work".into() });
    }

    #[test]
    fn list_profiles_roundtrip() { roundtrip_client(ClientMessage::ListProfiles); }

    #[test]
    fn pong_roundtrip() { roundtrip_daemon(DaemonMessage::Pong); }

    #[test]
    fn started_roundtrip() { roundtrip_daemon(DaemonMessage::Started { duration: 10 }); }

    #[test]
    fn status_with_time_roundtrip() {
        roundtrip_daemon(DaemonMessage::StatusWithTime {
            time_left: 10,
            sites: vec!["reddit.com".into()],
        });
    }

    #[test]
    fn status_idle_roundtrip() {
        roundtrip_daemon(DaemonMessage::StatusIdle { sites: vec![] });
    }

    #[test]
    fn stopped_roundtrip() { roundtrip_daemon(DaemonMessage::Stopped); }

    #[test]
    fn global_website_list_roundtrip() {
        roundtrip_daemon(DaemonMessage::GlobalWebsiteList(
            vec!["reddit.com".into(), "youtube.com".into()],
        ));
    }

    #[test]
    fn error_roundtrip() { roundtrip_daemon(DaemonMessage::Error("test".into())); }

    #[test]
    fn manual_session_none_roundtrip() {
        roundtrip_daemon(DaemonMessage::ManualSession(None));
    }

    #[test]
    fn active_policy_roundtrip() {
        roundtrip_daemon(DaemonMessage::ActivePolicy(ActivePolicy::default()));
    }

    #[test]
    fn decode_invalid_bytes_returns_error() {
        let result = decode::<DaemonMessage>(&[0x80, 0x01, 0x02]);
        assert!(result.is_err());
    }
}
