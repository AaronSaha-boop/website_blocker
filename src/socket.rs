// src/socket.rs

use std::io;
use std::path::{Path, PathBuf};
use tokio::net::{UnixListener, UnixStream};
use thiserror::Error;
use tokio_util::codec::{Framed, LengthDelimitedCodec};
use futures::{SinkExt, StreamExt};
use bytes::Bytes;
use tokio::time::{timeout, Duration};

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

    #[error("Operation timed out after {} seconds", timeout.as_secs())]
    Timeout { timeout: Duration },
}

/// A framed connection for sending/receiving length-prefixed messages
pub struct Connection {
    framed: Framed<UnixStream, LengthDelimitedCodec>,
    timeout: Duration,
}

impl Connection {
    pub fn new(stream: UnixStream, timeout: Duration) -> Self {
        Self {
            framed: Framed::new(
                stream, 
                LengthDelimitedCodec::new()),
            timeout,
        }
    }

    pub async fn send(&mut self, data: &[u8]) -> Result<(), SocketError> {
        timeout(self.timeout, self.framed.send(Bytes::copy_from_slice(data)))
            .await
            .map_err(|_| SocketError::Timeout { timeout: self.timeout })?
            .map_err(|e| SocketError::ConnectionError { source: e })
    }

    pub async fn recv(&mut self) -> Result<Option<Bytes>, SocketError> {
        match timeout(self.timeout, self.framed.next()).await {
            // Return the 'Bytes' object directly (no copy)
            Ok(Some(Ok(bytes))) => Ok(Some(bytes.freeze())), 
            Ok(Some(Err(e))) => Err(SocketError::ConnectionError { source: e }),
            Ok(None) => Ok(None),
            Err(_) => Err(SocketError::Timeout { timeout: self.timeout }),
        }
    }

    pub async fn shutdown(&mut self) -> Result<(), SocketError> {
        self.framed.close().await
            .map_err(|e| SocketError::ConnectionError { source: e })
    }
}

/// Unix socket server for daemon IPC
pub struct SocketServer {
    listener: UnixListener,
    path: PathBuf,
}

impl SocketServer {
    pub async fn bind(path: impl AsRef<Path>) -> Result<Self, SocketError> {
        let path = path.as_ref().to_path_buf();
        let _ = std::fs::remove_file(&path);
        
        let listener = UnixListener::bind(&path)
            .map_err(|e| SocketError::BindFailed { 
                path: path.clone(), 
                source: e 
            })?;
        
        Ok(Self { listener, path })
    }

    pub async fn accept(&self, timeout: Duration) -> Result<Connection, SocketError> {
        let (stream, _addr) = self.listener.accept().await
            .map_err(|e| SocketError::AcceptFailed { source: e })?;
        
        Ok(Connection::new(stream, timeout))
    }

    pub async fn accept_no_timeout(&self, conn_timeout: Duration) -> Result<Connection, SocketError> {
        let (stream, _addr) = self.listener.accept().await
            .map_err(|e| SocketError::AcceptFailed { source: e })?;
        Ok(Connection::new(stream, conn_timeout))
    }

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
    use tempfile::tempdir;

    use super::*;

    struct TestConnectionPair {
        client: Connection,
        server: Connection,
    }

    impl TestConnectionPair {
        fn new() -> Self {
            let (client_stream, server_stream) = UnixStream::pair().unwrap();
            Self {
                // Short timeout for tests
                client: Connection::new(client_stream, Duration::from_secs(10)),
                server: Connection::new(server_stream, Duration::from_secs(10)),
            }
        }
    }

    #[tokio::test]
    async fn connection_send_recv_roundtrip() {
        let mut pair = TestConnectionPair::new();
        
        pair.client.send(b"hello").await.unwrap();
        
        let received = pair.server.recv().await.unwrap().unwrap();
        assert_eq!(&received[..], b"hello");
    }
    // Message framing tests

    #[tokio::test]
    async fn connection_recv_returns_none_on_close() {
        let mut pair = TestConnectionPair::new();
        
        // Client connects then drops
        pair.client.shutdown().await.unwrap();
        
        let result = pair.server.recv().await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn connection_send_recv_empty_message() {
        let mut pair = TestConnectionPair::new();
        
        pair.client.send(b"").await.unwrap();
        
        let received = pair.server.recv().await.unwrap().unwrap();
        assert_eq!(&received[..], b"");
    }

    #[tokio::test]
    async fn connection_send_recv_large_message() {
        let mut pair = TestConnectionPair::new();
        
        let large_data: Vec<u8> = (0..1_000).map(|i| (i % 256) as u8).collect();
        
        pair.client.send(&large_data).await.unwrap();
        
        let received = pair.server.recv().await.unwrap().unwrap();
        assert_eq!(&received[..], &large_data[..]);
    }

    // SocketServer tests
    #[tokio::test]
    async fn bind_creates_socket_file() { 
        let dir = tempdir().unwrap();
        let socket_path = dir.path().join("test.sock");

        let _server = SocketServer::bind(&socket_path).await.unwrap();
        
        assert!(socket_path.exists());
    }

    #[tokio::test]
    async fn bind_removes_existing_socket() { 
        let dir = tempdir().unwrap();
        let socket_path = dir.path().join("test.sock");
        
        // Create a dummy file
        std::fs::write(&socket_path, "dummy content").unwrap();
        assert!(socket_path.exists());
        
        // Bind should remove old file and create new socket
        let server = SocketServer::bind(&socket_path).await;
        assert!(server.is_ok());
        
        // Socket file should exist (new socket created)
        assert!(socket_path.exists());

    }

    #[tokio::test]
    async fn accept_returns_stream() { 
        let dir = tempdir().unwrap();
        let socket_path = dir.path().join("test.sock");

        let server = SocketServer::bind(&socket_path).await.unwrap();

        // Connect from another task
        let client_handle = tokio::spawn(async move {
            UnixStream::connect(socket_path).await
        });

        let mut conn = server.accept(Duration::from_secs(1)).await.unwrap();
        let client_stream = client_handle.await.unwrap().unwrap();
        
        // Verify they can talk
        let mut client_conn = Connection::new(client_stream, Duration::from_secs(1));
        client_conn.send(b"ping").await.unwrap();
        let received = conn.recv().await.unwrap().unwrap();
        assert_eq!(&received[..], b"ping");
    }

    #[tokio::test]
    async fn drop_removes_socket_file() { 
        let dir = tempdir().unwrap();
        let socket_path = dir.path().join("test.sock");

        let existing_server = SocketServer::bind(&socket_path).await.unwrap();

        drop(existing_server);

        assert!(!socket_path.exists());
    }

    // Integration test
    #[tokio::test]
    async fn client_server_communication() { 
        let dir = tempdir().unwrap();
        let socket_path = dir.path().join("test.sock");

        let server = SocketServer::bind(&socket_path).await.unwrap();

        let server_handle = tokio::spawn(async move {
            let mut conn = server.accept(Duration::from_secs(1)).await.unwrap();
            let msg = conn.recv().await.unwrap().unwrap();
            conn.send(&msg).await.unwrap();
        });

        let client_stream = UnixStream::connect(socket_path).await.unwrap();
        let mut client_conn = Connection::new(client_stream, Duration::from_secs(1));
        
        client_conn.send(b"hello").await.unwrap();
        let response = client_conn.recv().await.unwrap().unwrap();
        assert_eq!(&response[..], b"hello");

        server_handle.await.unwrap();
    }
}