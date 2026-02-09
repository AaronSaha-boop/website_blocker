// src/socket.rs

use std::io;
use std::path::{Path, PathBuf};
use tokio::net::{UnixListener, UnixStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SocketError {
    #[error("Failed to bind socket at {}: {source}", path.display())]
    BindFailed { path: PathBuf, source: io::Error },

    #[error("Failed to accept connection: {source}")]
    AcceptFailed { source: io::Error },

    #[error("Connection error: {source}")]
    ConnectionError { source: io::Error },

    #[error("Protocol error: {message}")]
    ProtocolError { message: String },
}

/// Send a length-prefixed message
pub async fn send_message(stream: &mut UnixStream, data: &[u8]) -> Result<(), SocketError> {
    todo!()
}

/// Receive a length-prefixed message
/// Returns None if connection is closed
pub async fn recv_message(stream: &mut UnixStream) -> Result<Option<Vec<u8>>, SocketError> {
    todo!()
}

/// Unix socket server for daemon IPC
pub struct SocketServer {
    listener: UnixListener,
    path: PathBuf,
}

impl SocketServer {
    /// Bind to a Unix socket path
    pub async fn bind(path: impl AsRef<Path>) -> Result<Self, SocketError> {
        todo!()
    }

    /// Accept a new connection
    pub async fn accept(&self) -> Result<UnixStream, SocketError> {
        todo!()
    }

    /// Get the socket path
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for SocketServer {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn send_recv_message_roundtrip() {
        let dir = tempdir().unwrap();
        let socket_path = dir.path().join("test.sock");
        
        // Create server
        let server = SocketServer::bind(&socket_path).await.unwrap();
        
        // Spawn client in background
        let client_path = socket_path.clone();
        let client_handle = tokio::spawn(async move {
            let mut client = UnixStream::connect(&client_path).await.unwrap();
            send_message(&mut client, b"hello").await.unwrap();
            
            let response = recv_message(&mut client).await.unwrap();
            response
        });
        
        // Server accepts and responds
        let mut stream = server.accept().await.unwrap();
        let received = recv_message(&mut stream).await.unwrap().unwrap();
        assert_eq!(received, b"hello");
        
        send_message(&mut stream, b"world").await.unwrap();
        
        // Check client received response
        let client_received = client_handle.await.unwrap().unwrap();
        assert_eq!(client_received, b"world");
    }
}