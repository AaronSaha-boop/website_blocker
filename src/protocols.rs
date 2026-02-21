// src/protocols.rs

use serde::{Serialize, Deserialize, de::DeserializeOwned};
use thiserror::Error;

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
    AddWebsite { url: String },
    RemoveWebsite { url: String },
    ListWebsites,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub enum DaemonMessage {
    Pong,
    Started { duration: u64 },
    StatusWithTime { time_left: u64 },
    StatusIdle,
    Stopped,
    WebsiteAdded { url: String },
    WebsiteRemoved { url: String },
    WebsiteList { websites: Vec<String> },
    Error(String),
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
    fn add_website_roundtrip() {
        roundtrip_client(ClientMessage::AddWebsite { url: "reddit.com".into() });
    }

    #[test]
    fn remove_website_roundtrip() {
        roundtrip_client(ClientMessage::RemoveWebsite { url: "reddit.com".into() });
    }

    #[test]
    fn list_websites_roundtrip() { roundtrip_client(ClientMessage::ListWebsites); }

    #[test]
    fn pong_roundtrip() { roundtrip_daemon(DaemonMessage::Pong); }

    #[test]
    fn started_roundtrip() { roundtrip_daemon(DaemonMessage::Started { duration: 10 }); }

    #[test]
    fn status_with_time_roundtrip() {
        roundtrip_daemon(DaemonMessage::StatusWithTime { time_left: 10 });
    }

    #[test]
    fn status_idle_roundtrip() { roundtrip_daemon(DaemonMessage::StatusIdle); }

    #[test]
    fn stopped_roundtrip() { roundtrip_daemon(DaemonMessage::Stopped); }

    #[test]
    fn website_list_roundtrip() {
        roundtrip_daemon(DaemonMessage::WebsiteList {
            websites: vec!["reddit.com".into(), "youtube.com".into()],
        });
    }

    #[test]
    fn error_roundtrip() { roundtrip_daemon(DaemonMessage::Error("test".into())); }

    #[test]
    fn decode_invalid_bytes_returns_error() {
        let result = decode::<DaemonMessage>(&[0x80, 0x01, 0x02]);
        assert!(result.is_err());
    }
}
