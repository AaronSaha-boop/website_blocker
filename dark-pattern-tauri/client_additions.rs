// ============================================================
// ADD THIS TO YOUR EXISTING src/client.rs
// ============================================================

use crate::protocols::{self, ClientMessage, DaemonMessage, ProtocolError};

/// Send a message and receive a response (reusable helper for CLI and Tauri)
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

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("Socket error: {0}")]
    Socket(#[from] SocketError),
    
    #[error("Protocol error: {0}")]
    Protocol(#[from] ProtocolError),
    
    #[error("Connection closed by daemon")]
    ConnectionClosed,
}
