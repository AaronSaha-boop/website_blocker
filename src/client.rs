use std::path::Path;
use tokio::net::UnixStream;
use tokio::time::Duration;
use crate::socket::{Connection, SocketError};

/// Connect to a Unix socket server
pub async fn connect(path: impl AsRef<Path>, timeout: Duration) -> Result<Connection, SocketError> {
    let path = path.as_ref();
    
    let stream = UnixStream::connect(path).await
        .map_err(|e| SocketError::ConnectionError { source: e })?;
    
    Ok(Connection::new(stream, timeout))
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

