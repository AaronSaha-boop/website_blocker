use std::path::{Path, PathBuf};
use thiserror::Error;
use tokio::{fs::{self, File}, io::AsyncWriteExt};
use std::io;

#[derive(Error, Debug)]
pub enum FsError {
    #[error("File not found: {}", path.display())]
    NotFound { path: PathBuf },

    #[error("Permission denied: {}", path.display())]
    PermissionDenied { path: PathBuf },

    #[error("Failed to read {}: {source}", path.display())]
    ReadError { 
        path: PathBuf, 
        #[source] source: io::Error,
    },

    #[error("Failed to write {}: {source}", path.display())]
    WriteError { 
        path: PathBuf, 
        #[source] source: io::Error,
    },

    #[error("Failed to delete {}: {source}", path.display())]
    DeleteError { 
        path: PathBuf, 
        #[source] source: io::Error,
    },

    #[error("Failed to set permissions for {}: {source}", path.display())]
    SetPermissionsError { 
        path: PathBuf, 
        #[source] source: io::Error,
    },

    #[error("Atomic write failed for {}: {reason}", path.display())]
    AtomicWriteFailed { 
        path: PathBuf, 
        reason: String,
    },
}

impl FsError {
    pub fn from_io_read_error(error: io::Error, path: PathBuf) -> Self {
        match error.kind() {
            io::ErrorKind::NotFound => FsError::NotFound { path },
            io::ErrorKind::PermissionDenied => FsError::PermissionDenied { path },
            _ => FsError::ReadError { path, source: error },
        }
    }

    pub fn from_io_write_error(error: io::Error, path: PathBuf) -> Self {
        match error.kind() {
            io::ErrorKind::NotFound => FsError::NotFound { path },
            io::ErrorKind::PermissionDenied => FsError::PermissionDenied { path },
            _ => FsError::WriteError { path, source: error },
        }
    }

    pub fn from_io_exist_error(error: io::Error, path: PathBuf) -> Self {
        match error.kind() {
            io::ErrorKind::NotFound => FsError::NotFound { path },
            io::ErrorKind::PermissionDenied => FsError::PermissionDenied { path },
            _ => FsError::ReadError { path, source: error },
        }
    }

    pub fn from_io_delete_error(error: io::Error, path: PathBuf) -> Self {
        match error.kind() {
            io::ErrorKind::NotFound => FsError::NotFound { path },
            io::ErrorKind::PermissionDenied => FsError::PermissionDenied { path },
            _ => FsError::DeleteError { path, source: error },
        }
    }

    pub fn from_io_permissions_error(error: io::Error, path: PathBuf) -> Self {
        match error.kind() {
            io::ErrorKind::NotFound => FsError::NotFound { path },
            io::ErrorKind::PermissionDenied => FsError::PermissionDenied { path },
            _ => FsError::SetPermissionsError { path, source: error },
        }
    }
}

/// Temporary file that cleans up on drop unless committed
struct TempFile {
    path: PathBuf,
    committed: bool,
}

impl TempFile {
    fn new(path: PathBuf) -> Self {
        Self { path, committed: false }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn commit(mut self) {
        self.committed = true;
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        if !self.committed {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

pub async fn read_file(path: impl AsRef<Path>) -> Result<String, FsError> {
    let path = path.as_ref();
    fs::read_to_string(path)
        .await
        .map_err(|e| FsError::from_io_read_error(e, path.to_path_buf()))
}

pub async fn write_file_atomic(path: impl AsRef<Path>, content: &str) -> Result<(), FsError> {
    let path = path.as_ref();
    let parent = path.parent().ok_or_else(|| FsError::AtomicWriteFailed { 
        path: path.to_path_buf(), 
        reason: "No parent directory".to_string(), 
    })?;

    let temp_path = parent.join(format!(".{}.tmp.{}",
        path.file_name().unwrap_or_default().to_string_lossy(),
        std::process::id()
    ));

    let temp_file = TempFile::new(temp_path.clone());

    let mut file = File::create(temp_file.path()).await.map_err(|e| {
        FsError::from_io_write_error(e, temp_path.clone())
    })?;

    file.write_all(content.as_bytes()).await.map_err(|e| {
        FsError::from_io_write_error(e, temp_path.clone())
    })?;

    file.flush().await.map_err(|e| {
        FsError::from_io_write_error(e, temp_path.clone())
    })?;

    file.sync_all().await.map_err(|e| {
        FsError::from_io_write_error(e, temp_path.clone())
    })?;

    fs::rename(temp_file.path(), path).await.map_err(|e| {
        FsError::from_io_write_error(e, path.to_path_buf())
    })?;

    temp_file.commit();

    Ok(())
}


/// Check if path exists and is a file
pub async fn file_exists(path: impl AsRef<Path>) -> Result<bool, FsError> {
    let path = path.as_ref();
    
    match fs::metadata(path).await {
        Ok(metadata) => Ok(metadata.is_file()),
        Err(e) => {
            if e.kind() == io::ErrorKind::NotFound {
                Ok(false)
            } else {
                Err(FsError::from_io_exist_error(e, path.to_path_buf()))
            }
        }
    }
}


/// Remove file if it exists, return whether file was removed
pub async fn remove_file_if_exists(path: impl AsRef<Path>) -> Result<bool, FsError> {
    let path = path.as_ref();
    match fs::remove_file(path).await {
        Ok(_) => Ok(true),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(FsError::from_io_delete_error(e, path.to_path_buf())),
    }
}
/// Set Unix file permissions
#[cfg(unix)]
pub async fn set_permissions(path: impl AsRef<Path>, mode: u32) -> Result<(), FsError> {
    use std::os::unix::fs::PermissionsExt;
    
    let path = path.as_ref();
    let permissions = std::fs::Permissions::from_mode(mode);
    
    fs::set_permissions(path, permissions)
        .await
        .map_err(|e| FsError::from_io_permissions_error(e, path.to_path_buf()))
}

#[cfg(test)]
mod tests {
    use std::{fs::exists, result};

    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn read_file_returns_content() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        
        // Setup: create file with known content
        tokio::fs::write(&file_path, "hello world").await.unwrap();
        
        // Test
        let content = read_file(&file_path).await.unwrap();
        
        // Assert
        assert_eq!(content, "hello world");
    }

    #[tokio::test]
    async fn read_file_nonexistent_returns_error() {
        let result = read_file("/nonexistent/path/file.txt").await;
        assert!(result.is_err());
    }

    // write_file_atomic tests
    #[tokio::test]
    async fn write_atomic_creates_new_file() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        let content = "hello friend";

        assert!(!file_path.exists());

        write_file_atomic(&file_path, content).await.unwrap();

        assert!(file_path.exists());
    }

    //create help function for test later 

    #[tokio::test]
    async fn write_atomic_overwrites_existing() { 
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        tokio::fs::write(&file_path, "hello world").await.unwrap();
        let content = "goodbye friend";

        write_file_atomic(&file_path, content).await.unwrap();

        let read_content = tokio::fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(read_content, content);
    }

    #[tokio::test]
    async fn write_atomic_content_is_correct() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        let content = "hello friend";

        write_file_atomic(&file_path, content).await.unwrap();

        let read_content = tokio::fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(read_content, content);
    }

    #[tokio::test]
    async fn write_atomic_cleans_up_temp_file() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        let content = "hello friend";

        write_file_atomic(&file_path, content).await.unwrap();

        // Check no .tmp files remain
        let mut entries = tokio::fs::read_dir(dir.path()).await.unwrap();
        
        while let Some(entry) = entries.next_entry().await.unwrap() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            assert!(!name_str.contains(".tmp"), "Temp file not cleaned up: {}", name_str);
        }
    }

    // file_exists tests
    #[tokio::test]
    async fn file_exists_returns_true_for_file() { 
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        
        tokio::fs::write(&file_path, "content").await.unwrap();
        
        let result = file_exists(&file_path).await.unwrap();
        assert!(result);
    }

    #[tokio::test]
    async fn file_exists_returns_false_for_nonexistent() {
        let result = file_exists("/nonexistent/path/file.txt").await.unwrap();
        assert!(!result);
    }

    #[tokio::test]
    async fn file_exists_returns_false_for_directory() {
        let dir = tempdir().unwrap();
        
        let result = file_exists(dir.path()).await.unwrap();
        assert!(!result);
    }

    // remove_file_if_exists tests
    #[tokio::test]
    async fn remove_file_if_exists_removes_file() { 
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");

        tokio::fs::write(&file_path, "content").await.unwrap();
        assert!(file_path.exists());

        let result = remove_file_if_exists(&file_path).await.unwrap();
        assert_eq!(result, true);
        assert!(!file_path.exists());
    }

    #[tokio::test]
    async fn remove_file_if_exists_nonexistent_returns_false() { 
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");

        assert!(!file_path.exists());

        let result = remove_file_if_exists(&file_path).await.unwrap();
        assert_eq!(result, false);
        assert!(!file_path.exists());
    }

    // set_permissions tests (Unix)
    #[tokio::test]
    #[cfg(unix)]
    async fn set_permissions_changes_mode() {
        use std::os::unix::fs::PermissionsExt;
        
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        
        tokio::fs::write(&file_path, "content").await.unwrap();
        
        // Set to 644 (rw-r--r--)
        set_permissions(&file_path, 0o644).await.unwrap();
        let meta = tokio::fs::metadata(&file_path).await.unwrap();
        assert_eq!(meta.permissions().mode() & 0o777, 0o644);
        
        // Set to 600 (rw-------)
        set_permissions(&file_path, 0o600).await.unwrap();
        let meta = tokio::fs::metadata(&file_path).await.unwrap();
        assert_eq!(meta.permissions().mode() & 0o777, 0o600);
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn set_permissions_nonexistent_returns_error() {
        let result = set_permissions("/nonexistent/path/file.txt", 0o644).await;
        assert!(matches!(result, Err(FsError::NotFound { .. })));
    }

}
