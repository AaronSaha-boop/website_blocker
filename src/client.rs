// src/client.rs
//
// Client for connecting to the daemon over Unix socket.

use std::path::Path;
use std::time::Duration;
use thiserror::Error;
use tokio::net::UnixStream;

use crate::protocols::{self, ClientMessage, DaemonMessage, ProtocolError};
use crate::socket::{Connection, SocketError};

const SOCKET_PATH: &str = "/tmp/dark-pattern.sock";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

// ─────────────────────────────────────────────────────────────────────────────
// Error
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("Socket error: {0}")]
    Socket(#[from] SocketError),

    #[error("Protocol error: {0}")]
    Protocol(#[from] ProtocolError),

    #[error("Connection closed by daemon")]
    ConnectionClosed,
}

// ─────────────────────────────────────────────────────────────────────────────
// Connection
// ─────────────────────────────────────────────────────────────────────────────

/// Connect to the daemon's Unix socket
pub async fn connect(path: impl AsRef<Path>, timeout: Duration) -> Result<Connection, SocketError> {
    let path = path.as_ref();
    let stream = UnixStream::connect(path)
        .await
        .map_err(|e| SocketError::ConnectionError { source: e })?;
    Ok(Connection::new(stream, timeout))
}

/// Send a message and receive a response over an existing connection
pub async fn send_and_receive(
    conn: &mut Connection,
    msg: ClientMessage,
) -> Result<DaemonMessage, ClientError> {
    let bytes = protocols::encode(&msg)?;
    conn.send(&bytes).await?;

    let response_bytes = conn
        .recv()
        .await?
        .ok_or(ClientError::ConnectionClosed)?;

    let response = protocols::decode(&response_bytes)?;
    Ok(response)
}

// ─────────────────────────────────────────────────────────────────────────────
// High-level API (used by Tauri commands)
// ─────────────────────────────────────────────────────────────────────────────

/// Connect, send message, return response. Converts DaemonMessage::Error to Err.
/// This is the main function used by Tauri commands.
pub async fn send_message(msg: ClientMessage) -> Result<DaemonMessage, String> {
    let mut conn = connect(SOCKET_PATH, CONNECT_TIMEOUT)
        .await
        .map_err(|_| "Failed to connect to daemon. Is it running?".to_string())?;

    let response = send_and_receive(&mut conn, msg)
        .await
        .map_err(|e| e.to_string())?;

    // Convert daemon-level errors to Err
    if let DaemonMessage::Error(ref err) = response {
        return Err(err.clone());
    }

    Ok(response)
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::socket::SocketServer;
    use tempfile::tempdir;

    #[tokio::test]
    async fn client_connects_to_server() {
        let dir = tempdir().unwrap();
        let socket_path = dir.path().join("test.sock");

        let server = SocketServer::bind(&socket_path).await.unwrap();

        let client_path = socket_path.clone();
        let client_handle = tokio::spawn(async move {
            connect(&client_path, Duration::from_secs(2)).await
        });

        let _server_conn = server.accept(Duration::from_secs(2)).await.unwrap();
        let client_result = client_handle.await.unwrap();
        assert!(client_result.is_ok());
    }
}
