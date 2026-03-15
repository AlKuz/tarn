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

    /// Verifies that file events through a symlinked path are correctly resolved.
    /// On macOS, /var is a symlink to /private/var, so watching a symlinked path
    /// must canonicalize to match the paths returned by the filesystem watcher.
    #[tokio::test]
    #[cfg(unix)]
    async fn symlinked_directory_events_work() {
        use std::os::unix::fs::symlink;

        let real_dir = TempDir::new().unwrap();
        let link_dir = TempDir::new().unwrap();
        let link_path = link_dir.path().join("link");

        // Create symlink: link_path -> real_dir
        symlink(real_dir.path(), &link_path).unwrap();

        // Watch via the symlink
        let observer = LocalStorageObserver::new(link_path.clone());
        let stream = observer.observe().await.unwrap();
        let mut stream = pin!(stream);

        tokio::time::sleep(Duration::from_millis(WATCHER_SETTLE_MS)).await;

        // Write file through the symlink
        fs::write(link_path.join("test.md"), "content")
            .await
            .unwrap();

        let event = timeout(Duration::from_secs(EVENT_TIMEOUT_SECS), stream.next())
            .await
            .expect("timeout")
            .expect("stream ended");

        match event {
            StorageEvent::Created { path, .. } => {
                assert_eq!(path, VaultPath::new("test.md").unwrap());
            }
            other => panic!("expected Created, got {:?}", other),
        }
    }

    /// Verifies that root directory events are filtered out.
    /// Some platforms include the watched root in events; we should skip these.
    #[tokio::test]
    async fn root_directory_events_are_filtered() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("subdir")).await.unwrap();

        let observer = LocalStorageObserver::new(dir.path().to_path_buf());
        let stream = observer.observe().await.unwrap();
        let mut stream = pin!(stream);

        tokio::time::sleep(Duration::from_millis(WATCHER_SETTLE_MS)).await;

        // Create file in subdirectory - some platforms emit root dir events
        fs::write(dir.path().join("subdir/file.md"), "content")
            .await
            .unwrap();

        let event = timeout(Duration::from_secs(EVENT_TIMEOUT_SECS), stream.next())
            .await
            .expect("timeout")
            .expect("stream ended");

        // The first event we receive must be for the actual file, not root
        match event {
            StorageEvent::Created { path, .. } => {
                assert!(
                    !path.as_str().is_empty(),
                    "received event for root directory (empty path)"
                );
                assert_eq!(path, VaultPath::new("subdir/file.md").unwrap());
            }
            other => panic!("expected Created, got {:?}", other),
        }
    }
}

// =============================================================================
// Index Sync
// =============================================================================

mod index_sync {
    use super::*;
    use tarn::TarnBuilder;

    const WATCHER_SETTLE_MS: u64 = 100;
    const SYNC_WAIT_MS: u64 = 500;

    #[tokio::test]
    async fn sync_indexes_new_file() {
        let dir = TempDir::new().unwrap();

        let core = TarnBuilder::local(dir.path().to_path_buf())
            .with_index("bert-base-uncased")
            .build_async()
            .await
            .unwrap();

        let _handle = core.start_index_sync().unwrap();

        // Give watcher time to initialize
        tokio::time::sleep(Duration::from_millis(WATCHER_SETTLE_MS)).await;

        // Create a new note
        fs::write(
            dir.path().join("test.md"),
            "# Hello\n\nThis is a test note about rust programming.",
        )
        .await
        .unwrap();

        // Wait for sync
        tokio::time::sleep(Duration::from_millis(SYNC_WAIT_MS)).await;

        // Verify index was updated via search
        let results = core
            .search_notes("rust programming", None, None, 10, 0)
            .await
            .unwrap();
        assert_eq!(results.total, 1);
        assert_eq!(results.results[0].path, "test.md");
    }

    #[tokio::test]
    async fn sync_updates_modified_file() {
        let dir = TempDir::new().unwrap();

        // Create initial file
        fs::write(
            dir.path().join("note.md"),
            "# Original\n\nOriginal content about apples.",
        )
        .await
        .unwrap();

        let core = TarnBuilder::local(dir.path().to_path_buf())
            .with_index("bert-base-uncased")
            .build_async()
            .await
            .unwrap();

        core.rebuild_index().await.unwrap();

        // Verify initial content is indexed
        let results = core
            .search_notes("apples", None, None, 10, 0)
            .await
            .unwrap();
        assert_eq!(results.total, 1);

        let _handle = core.start_index_sync().unwrap();
        tokio::time::sleep(Duration::from_millis(WATCHER_SETTLE_MS)).await;

        // Modify the file with different content
        fs::write(
            dir.path().join("note.md"),
            "# Updated\n\nUpdated content about oranges.",
        )
        .await
        .unwrap();

        tokio::time::sleep(Duration::from_millis(SYNC_WAIT_MS)).await;

        // Verify old content is gone, new content is indexed
        let old_results = core
            .search_notes("apples", None, None, 10, 0)
            .await
            .unwrap();
        assert_eq!(old_results.total, 0);

        let new_results = core
            .search_notes("oranges", None, None, 10, 0)
            .await
            .unwrap();
        assert_eq!(new_results.total, 1);
    }

    #[tokio::test]
    async fn sync_removes_deleted_file() {
        let dir = TempDir::new().unwrap();

        // Create initial file
        fs::write(
            dir.path().join("to_delete.md"),
            "# Delete Me\n\nUnique deleteme content.",
        )
        .await
        .unwrap();

        let core = TarnBuilder::local(dir.path().to_path_buf())
            .with_index("bert-base-uncased")
            .build_async()
            .await
            .unwrap();

        core.rebuild_index().await.unwrap();

        // Verify file is indexed
        let results = core
            .search_notes("deleteme", None, None, 10, 0)
            .await
            .unwrap();
        assert_eq!(results.total, 1);

        let _handle = core.start_index_sync().unwrap();
        tokio::time::sleep(Duration::from_millis(WATCHER_SETTLE_MS)).await;

        // Delete the file
        fs::remove_file(dir.path().join("to_delete.md"))
            .await
            .unwrap();

        tokio::time::sleep(Duration::from_millis(SYNC_WAIT_MS)).await;

        // Verify file is no longer in index
        let results = core
            .search_notes("deleteme", None, None, 10, 0)
            .await
            .unwrap();
        assert_eq!(results.total, 0);
    }

    #[tokio::test]
    async fn sync_ignores_non_markdown_files() {
        let dir = TempDir::new().unwrap();

        let core = TarnBuilder::local(dir.path().to_path_buf())
            .with_index("bert-base-uncased")
            .build_async()
            .await
            .unwrap();

        let _handle = core.start_index_sync().unwrap();
        tokio::time::sleep(Duration::from_millis(WATCHER_SETTLE_MS)).await;

        // Create non-markdown files
        fs::write(dir.path().join("image.png"), "fake image data")
            .await
            .unwrap();
        fs::write(dir.path().join("data.json"), "{}").await.unwrap();

        tokio::time::sleep(Duration::from_millis(SYNC_WAIT_MS)).await;

        // Verify vault info shows 0 notes
        let info = core.vault_info(None).await.unwrap();
        assert_eq!(info.note_count, 0);
    }
}
