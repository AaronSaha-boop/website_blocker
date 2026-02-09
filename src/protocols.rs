// src/protocol.rs

use serde::{Serialize, Deserialize, de::DeserializeOwned};


#[derive(Debug, PartialEq)]
pub enum ProtocolError {
    EncodeError(String),
    DecodeError(String),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub enum ClientMessage {
    Ping,
    Start { duration: u64 },
    Stop,
    GetStatus,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub enum DaemonMessage {
    Pong,
    Started { duration: u64 },
    StatusWithTime { time_left: u64 },
    StatusIdle,
    Stopped,
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

    // ClientMessage tests
    #[test]
    fn ping_roundtrip() { 
        // * Ping → encode → decode → equals Ping
        let msg = ClientMessage::Ping;
        let bytes = encode(&msg).unwrap();
        let decoded = decode::<ClientMessage>(&bytes).unwrap();
        assert_eq!(decoded, msg);
     }

    #[test]
    fn start_roundtrip() { 
        // * Start { duration: 10 } → encode → decode → equals Start { duration: 10 }
        let msg = ClientMessage::Start { duration: 10 };
        let bytes = encode(&msg).unwrap();
        let decoded = decode::<ClientMessage>(&bytes).unwrap();
        assert_eq!(decoded, msg);
     }

    #[test]
    fn stop_roundtrip() { 
        // * Stop → encode → decode → equals Stop
        let msg = ClientMessage::Stop;
        let bytes = encode(&msg).unwrap();
        let decoded = decode::<ClientMessage>(&bytes).unwrap();
        assert_eq!(decoded, msg);
     }

    #[test]
    fn get_status_roundtrip() { 
        // * GetStatus → encode → decode → equals GetStatus
        let msg = ClientMessage::GetStatus;
        let bytes = encode(&msg).unwrap();
        let decoded = decode::<ClientMessage>(&bytes).unwrap();
        assert_eq!(decoded, msg);
     }

    fn test_helper<T: Serialize + DeserializeOwned>(msg: &DaemonMessage) -> DaemonMessage {
        let bytes = encode(msg).unwrap();
        let decoded = decode::<DaemonMessage>(&bytes).unwrap();
        decoded
    }

    // DaemonMessage tests
    #[test]
    fn pong_roundtrip() { 
        // * Pong → encode → decode → equals Pong
        let msg = DaemonMessage::Pong;
        let decoded = test_helper::<DaemonMessage>(&msg);
        assert_eq!(decoded, msg);
     }

    #[test]
    fn started_roundtrip() { 
        // * Started { duration: 10 } → encode → decode → equals Started { duration: 10 }
        let msg = DaemonMessage::Started { duration: 10 };
        let decoded = test_helper::<DaemonMessage>(&msg);
        assert_eq!(decoded, msg);
     }

    #[test]
    fn status_with_time_roundtrip() { 
        // * StatusWithTime { time_left: 10 } → encode → decode → equals StatusWithTime { time_left: 10 }
        let msg = DaemonMessage::StatusWithTime { time_left: 10 };
        let decoded = test_helper::<DaemonMessage>(&msg);
        assert_eq!(decoded, msg);
     }

    #[test]
    fn status_idle_roundtrip() { 
        // * StatusIdle → encode → decode → equals StatusIdle
        let msg = DaemonMessage::StatusIdle;
        let decoded = test_helper::<DaemonMessage>(&msg);
        assert_eq!(decoded, msg);
     }

    #[test]
    fn stopped_roundtrip() { 
        // * Stopped → encode → decode → equals Stopped
        let msg = DaemonMessage::Stopped;
        let decoded = test_helper::<DaemonMessage>(&msg);
        assert_eq!(decoded, msg);
     }

    #[test]
    fn error_roundtrip() { 
        // * Error("test") → encode → decode → equals Error("test")
        let msg = DaemonMessage::Error("test".to_string());
        let decoded = test_helper::<DaemonMessage>(&msg);
        assert_eq!(decoded, msg);
     }  

    // Error cases
    #[test]
    fn decode_invalid_bytes_returns_error() {
        // * Invalid bytes → decode → returns error
        let bytes = vec![0x80, 0x01, 0x02];
        let result = decode::<DaemonMessage>(&bytes);
        assert!(result.is_err());
     }
}

