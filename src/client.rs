use std::path::Path;
use tokio::net::UnixStream;
use tokio::time::Duration;
use crate::protocols::{self, ClientMessage, DaemonMessage, ProtocolError};
use crate::socket::{Connection, SocketError};

const SOCKET_PATH: &str = "/tmp/dark-pattern.sock";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

// ─────────────────────────────────────────────────────────────────────────────
// Error type (used by the connection-based send_and_receive)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("Socket error: {0}")]
    Socket(#[from] SocketError),

    #[error("Protocol error: {0}")]
    Protocol(#[from] ProtocolError),

    #[error("Connection closed by daemon")]
    ConnectionClosed,
}

// ─────────────────────────────────────────────────────────────────────────────
// Core API
// ─────────────────────────────────────────────────────────────────────────────

/// Connect to a Unix socket server
pub async fn connect(path: impl AsRef<Path>, timeout: Duration) -> Result<Connection, SocketError> {
    let path = path.as_ref();

    let stream = UnixStream::connect(path).await
        .map_err(|e| SocketError::ConnectionError { source: e })?;

    Ok(Connection::new(stream, timeout))
}

/// Send a message and receive a response over an existing connection.
/// This is the low-level helper used by both the CLI and the Tauri app.
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

/// Convenience wrapper: connect to the daemon, send a message, return the response.
/// Converts DaemonMessage::Error into Err(String) for easy use in Tauri commands.
pub async fn send_message(msg: ClientMessage) -> Result<DaemonMessage, String> {
    let mut conn = connect(SOCKET_PATH, CONNECT_TIMEOUT)
        .await
        .map_err(|_| "Failed to connect to daemon. Is it running?".to_string())?;

    let response = send_and_receive(&mut conn, msg)
        .await
        .map_err(|e| e.to_string())?;

    if let DaemonMessage::Error(ref err) = response {
        return Err(err.clone());
    }

    Ok(response)
}

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
        
        // Client connects
        let client_path = socket_path.clone();
        let client_handle = tokio::spawn(async move {
            connect(&client_path, Duration::from_secs(2)).await
        });
        
        // Server accepts
        let _server_conn = server.accept(Duration::from_secs(2)).await.unwrap();
        
        // Client should have connected successfully
        let client_result = client_handle.await.unwrap();
        assert!(client_result.is_ok());
    }
    #[tokio::test]
    async fn client_server_full_communication() {
        let dir = tempdir().unwrap();
        let socket_path = dir.path().join("test.sock");
        
        let server = SocketServer::bind(&socket_path).await.unwrap();
        
        let client_path = socket_path.clone();
        let client_handle = tokio::spawn(async move {
            let mut conn = connect(&client_path, Duration::from_secs(2)).await.unwrap();
            
            conn.send(b"ping").await.unwrap();
            let response = conn.recv().await.unwrap().unwrap();
            response
        });
        
        let mut server_conn = server.accept(Duration::from_secs(2)).await.unwrap();
        
        let received = server_conn.recv().await.unwrap().unwrap();
        assert_eq!(&received[..], b"ping");
        
        server_conn.send(b"pong").await.unwrap();
        
        let client_response = client_handle.await.unwrap();
        assert_eq!(&client_response[..], b"pong");
    }
}

