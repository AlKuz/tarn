//! LocalStorageObserver integration tests focusing on corner cases.

use std::path::PathBuf;
use std::pin::pin;
use std::time::Duration;

use tempfile::TempDir;
use tokio::fs;
use tokio::time::timeout;
use tokio_stream::StreamExt;

use tarn::common::VaultPath;
use tarn::observer::{LocalStorageObserver, Observer, ObserverError, StorageEvent};

// =============================================================================
// Watcher Initialization
// =============================================================================

mod init {
    use super::*;

    #[tokio::test]
    async fn watch_nonexistent_directory_fails() {
        let observer = LocalStorageObserver::new(PathBuf::from("/nonexistent/path/12345"));

        let result = observer.observe().await;

        assert!(matches!(result, Err(ObserverError::WatchFailed(_, _))));
    }

    #[tokio::test]
    async fn watch_valid_directory_succeeds() {
        let dir = TempDir::new().unwrap();
        let observer = LocalStorageObserver::new(dir.path().to_path_buf());

        let result = observer.observe().await;

        assert!(result.is_ok());
    }
}

// =============================================================================
// Event Detection
// =============================================================================

mod events {
    use super::*;

    const WATCHER_SETTLE_MS: u64 = 100;
    const EVENT_TIMEOUT_SECS: u64 = 5;

    #[tokio::test]
    async fn detects_file_creation() {
        let dir = TempDir::new().unwrap();
        let observer = LocalStorageObserver::new(dir.path().to_path_buf());

        let stream = observer.observe().await.unwrap();
        let mut stream = pin!(stream);

        // Give watcher time to initialize
        tokio::time::sleep(Duration::from_millis(WATCHER_SETTLE_MS)).await;

        // Create file after starting observation
        let file_path = dir.path().join("new.md");
        fs::write(&file_path, "content").await.unwrap();

        let event = timeout(Duration::from_secs(EVENT_TIMEOUT_SECS), stream.next())
            .await
            .expect("timeout waiting for event")
            .expect("stream ended");

        match event {
            StorageEvent::Created { path, token } => {
                assert_eq!(path, VaultPath::new("new.md").unwrap());
                assert!(!token.to_string().is_empty());
            }
            other => panic!("expected Created, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn detects_file_modification() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("existing.md");
        fs::write(&file_path, "initial").await.unwrap();

        // Ensure file system settles before starting watcher
        tokio::time::sleep(Duration::from_millis(WATCHER_SETTLE_MS)).await;

        let observer = LocalStorageObserver::new(dir.path().to_path_buf());
        let stream = observer.observe().await.unwrap();
        let mut stream = pin!(stream);

        // Give watcher time to initialize
        tokio::time::sleep(Duration::from_millis(WATCHER_SETTLE_MS)).await;

        // Modify file
        fs::write(&file_path, "modified content").await.unwrap();

        let event = timeout(Duration::from_secs(EVENT_TIMEOUT_SECS), stream.next())
            .await
            .expect("timeout waiting for event")
            .expect("stream ended");

        match event {
            StorageEvent::Updated { path, .. } | StorageEvent::Created { path, .. } => {
                // Some filesystems report modify as create
                assert_eq!(path, VaultPath::new("existing.md").unwrap());
            }
            StorageEvent::Deleted { .. } => panic!("unexpected delete event"),
        }
    }

    #[tokio::test]
    async fn detects_file_deletion() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("to_delete.md");
        fs::write(&file_path, "content").await.unwrap();

        tokio::time::sleep(Duration::from_millis(WATCHER_SETTLE_MS)).await;

        let observer = LocalStorageObserver::new(dir.path().to_path_buf());
        let stream = observer.observe().await.unwrap();
        let mut stream = pin!(stream);

        // Give watcher time to initialize
        tokio::time::sleep(Duration::from_millis(WATCHER_SETTLE_MS)).await;

        // Delete file
        fs::remove_file(&file_path).await.unwrap();

        let event = timeout(Duration::from_secs(EVENT_TIMEOUT_SECS), stream.next())
            .await
            .expect("timeout waiting for event")
            .expect("stream ended");

        match event {
            StorageEvent::Deleted { path } => {
                assert_eq!(path, VaultPath::new("to_delete.md").unwrap());
            }
            other => panic!("expected Deleted, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn nested_file_events_have_relative_paths() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("sub/folder"))
            .await
            .unwrap();

        let observer = LocalStorageObserver::new(dir.path().to_path_buf());
        let stream = observer.observe().await.unwrap();
        let mut stream = pin!(stream);

        // Give watcher time to initialize
        tokio::time::sleep(Duration::from_millis(WATCHER_SETTLE_MS)).await;

        fs::write(dir.path().join("sub/folder/deep.md"), "content")
            .await
            .unwrap();

        let event = timeout(Duration::from_secs(EVENT_TIMEOUT_SECS), stream.next())
            .await
            .expect("timeout")
            .expect("stream ended");

        match event {
            StorageEvent::Created { path, .. } => {
                assert_eq!(path, VaultPath::new("sub/folder/deep.md").unwrap());
                // VaultPath always stores relative paths (no leading /)
                assert!(!path.as_str().starts_with('/'));
            }
            other => panic!("expected Created, got {:?}", other),
        }
    }
}
