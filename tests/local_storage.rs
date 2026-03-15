//! LocalStorage integration tests focusing on corner cases.

use std::pin::pin;
use std::time::Duration;

use tempfile::TempDir;
use tokio::fs;
use tokio_stream::StreamExt;

use tarn::common::VaultPath;
use tarn::storage::{FileContent, LocalStorage, Storage, StorageError};

fn create_temp_storage() -> (TempDir, LocalStorage) {
    let dir = TempDir::new().unwrap();
    let storage = LocalStorage::new(dir.path().to_path_buf());
    (dir, storage)
}

// =============================================================================
// Path Safety
// =============================================================================

mod path_safety {
    use super::*;

    #[tokio::test]
    async fn rejects_path_traversal() {
        // VaultPath itself rejects path traversal
        let result = VaultPath::new("../etc/passwd");
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn read_nonexistent_returns_not_found() {
        let (_dir, storage) = create_temp_storage();
        let path = VaultPath::new("nonexistent.md").unwrap();

        let result = storage.read(&path).await;

        assert!(matches!(result, Err(StorageError::NotFound(_))));
    }
}

// =============================================================================
// Revision Token Conflicts
// =============================================================================

mod revision_conflicts {
    use super::*;

    #[tokio::test]
    async fn write_with_stale_token_fails() {
        let (dir, storage) = create_temp_storage();
        let path = VaultPath::new("note.md").unwrap();

        // Create initial file
        let token1 = storage
            .write(
                &path,
                FileContent::Markdown {
                    content: "v1".to_string(),
                    token: "ignored".into(),
                },
            )
            .await
            .unwrap();

        // Modify file externally to change its mtime
        tokio::time::sleep(Duration::from_millis(10)).await;
        fs::write(dir.path().join(path.as_str()), "external change")
            .await
            .unwrap();

        // Try to write with old token
        let result = storage
            .write(
                &path,
                FileContent::Markdown {
                    content: "v2".to_string(),
                    token: token1,
                },
            )
            .await;

        assert!(matches!(result, Err(StorageError::Conflict(_, _, _))));
    }

    #[tokio::test]
    async fn delete_with_stale_token_fails() {
        let (dir, storage) = create_temp_storage();
        let path = VaultPath::new("note.md").unwrap();

        let token1 = storage
            .write(
                &path,
                FileContent::Markdown {
                    content: "content".to_string(),
                    token: "ignored".into(),
                },
            )
            .await
            .unwrap();

        // External modification
        tokio::time::sleep(Duration::from_millis(10)).await;
        fs::write(dir.path().join(path.as_str()), "changed")
            .await
            .unwrap();

        let result = storage.delete(&path, token1).await;

        assert!(matches!(result, Err(StorageError::Conflict(_, _, _))));
    }

    #[tokio::test]
    async fn rename_with_stale_token_fails() {
        let (dir, storage) = create_temp_storage();
        let from = VaultPath::new("old.md").unwrap();
        let to = VaultPath::new("new.md").unwrap();

        let token1 = storage
            .write(
                &from,
                FileContent::Markdown {
                    content: "content".to_string(),
                    token: "ignored".into(),
                },
            )
            .await
            .unwrap();

        tokio::time::sleep(Duration::from_millis(10)).await;
        fs::write(dir.path().join(from.as_str()), "changed")
            .await
            .unwrap();

        let result = storage.rename(&from, &to, token1).await;

        assert!(matches!(result, Err(StorageError::Conflict(_, _, _))));
    }
}

// =============================================================================
// Directory Creation
// =============================================================================

mod directory_creation {
    use super::*;

    #[tokio::test]
    async fn write_creates_nested_parents() {
        let (_dir, storage) = create_temp_storage();
        let path = VaultPath::new("a/b/c/deep.md").unwrap();

        let result = storage
            .write(
                &path,
                FileContent::Markdown {
                    content: "deep content".to_string(),
                    token: "ignored".into(),
                },
            )
            .await;

        assert!(result.is_ok());
        assert!(storage.is_exists(&path).await.unwrap());
    }

    #[tokio::test]
    async fn rename_creates_target_parents() {
        let (_dir, storage) = create_temp_storage();
        let from = VaultPath::new("source.md").unwrap();
        let to = VaultPath::new("nested/target.md").unwrap();

        let token = storage
            .write(
                &from,
                FileContent::Markdown {
                    content: "content".to_string(),
                    token: "ignored".into(),
                },
            )
            .await
            .unwrap();

        storage.rename(&from, &to, token).await.unwrap();

        assert!(!storage.is_exists(&from).await.unwrap());
        assert!(storage.is_exists(&to).await.unwrap());
    }

    #[tokio::test]
    async fn copy_creates_target_parents() {
        let (_dir, storage) = create_temp_storage();
        let from = VaultPath::new("source.md").unwrap();
        let to = VaultPath::new("deep/nested/copy.md").unwrap();

        storage
            .write(
                &from,
                FileContent::Markdown {
                    content: "content".to_string(),
                    token: "ignored".into(),
                },
            )
            .await
            .unwrap();

        let result = storage.copy(&from, &to).await;

        assert!(result.is_ok());
        assert!(storage.is_exists(&from).await.unwrap());
        assert!(storage.is_exists(&to).await.unwrap());
    }
}

// =============================================================================
// File Type Detection
// =============================================================================

mod file_types {
    use super::*;

    #[tokio::test]
    async fn file_without_extension_read_as_markdown() {
        let (dir, storage) = create_temp_storage();
        let path = VaultPath::new("README").unwrap();

        fs::write(dir.path().join(path.as_str()), "# No Extension")
            .await
            .unwrap();

        let content = storage.read(&path).await.unwrap();

        assert!(matches!(content, FileContent::Markdown { .. }));
    }

    #[tokio::test]
    async fn png_read_as_image() {
        let (dir, storage) = create_temp_storage();
        let path = VaultPath::new("image.png").unwrap();

        // Minimal valid PNG (1x1 transparent)
        let png_bytes: [u8; 67] = [
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48,
            0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00,
            0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78,
            0x9C, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00,
            0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
        ];
        fs::write(dir.path().join(path.as_str()), &png_bytes)
            .await
            .unwrap();

        let content = storage.read(&path).await.unwrap();

        match content {
            FileContent::Image { content: uri, .. } => {
                let decoded = uri.decode().unwrap();
                assert_eq!(decoded, png_bytes);
            }
            _ => panic!("expected Image content"),
        }
    }

    #[tokio::test]
    async fn unknown_extension_read_as_markdown() {
        let (dir, storage) = create_temp_storage();
        let path = VaultPath::new("data.xyz").unwrap();

        fs::write(dir.path().join(path.as_str()), "some data")
            .await
            .unwrap();

        let content = storage.read(&path).await.unwrap();

        assert!(matches!(content, FileContent::Markdown { .. }));
    }
}

// =============================================================================
// Error Handling
// =============================================================================

mod error_handling {
    use super::*;

    #[tokio::test]
    async fn read_missing_file_returns_not_found() {
        let (_dir, storage) = create_temp_storage();
        let path = VaultPath::new("nonexistent.md").unwrap();

        let result = storage.read(&path).await;

        assert!(matches!(result, Err(StorageError::NotFound(_))));
    }

    #[tokio::test]
    async fn delete_missing_file_returns_not_found() {
        let (_dir, storage) = create_temp_storage();
        let path = VaultPath::new("nonexistent.md").unwrap();

        let result = storage.delete(&path, "any".into()).await;

        assert!(matches!(result, Err(StorageError::NotFound(_))));
    }

    #[tokio::test]
    async fn copy_missing_source_returns_not_found() {
        let (_dir, storage) = create_temp_storage();
        let from = VaultPath::new("missing.md").unwrap();
        let to = VaultPath::new("target.md").unwrap();

        let result = storage.copy(&from, &to).await;

        assert!(matches!(result, Err(StorageError::NotFound(_))));
    }

    #[tokio::test]
    async fn is_exists_missing_returns_false_not_error() {
        let (_dir, storage) = create_temp_storage();
        let path = VaultPath::new("nonexistent.md").unwrap();

        let exists = storage.is_exists(&path).await.unwrap();

        assert!(!exists);
    }
}

// =============================================================================
// Listing
// =============================================================================

mod listing {
    use super::*;

    #[tokio::test]
    async fn list_empty_directory() {
        let (_dir, storage) = create_temp_storage();

        let stream = storage.list().await.unwrap();
        let stream = pin!(stream);
        let files: Vec<_> = stream.collect().await;

        assert!(files.is_empty());
    }

    #[tokio::test]
    async fn list_includes_nested_files() {
        let (dir, storage) = create_temp_storage();

        fs::create_dir_all(dir.path().join("a/b")).await.unwrap();
        fs::write(dir.path().join("root.md"), "").await.unwrap();
        fs::write(dir.path().join("a/level1.md"), "").await.unwrap();
        fs::write(dir.path().join("a/b/level2.md"), "")
            .await
            .unwrap();

        let stream = storage.list().await.unwrap();
        let stream = pin!(stream);
        let mut files: Vec<_> = stream.collect().await;
        files.sort();

        assert_eq!(files.len(), 3);
        assert!(files.iter().any(|p| p.ends_with("root.md")));
        assert!(files.iter().any(|p| p.ends_with("level1.md")));
        assert!(files.iter().any(|p| p.ends_with("level2.md")));
    }

    #[tokio::test]
    async fn list_excludes_directories() {
        let (dir, storage) = create_temp_storage();

        fs::create_dir_all(dir.path().join("subdir")).await.unwrap();
        fs::write(dir.path().join("file.md"), "").await.unwrap();

        let stream = storage.list().await.unwrap();
        let stream = pin!(stream);
        let files: Vec<_> = stream.collect().await;

        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with("file.md"));
    }
}

// =============================================================================
// DataURI: Parsing Edge Cases
// =============================================================================

mod data_uri {
    use std::str::FromStr;
    use tarn::common::DataURI;

    #[test]
    fn parse_missing_prefix_fails() {
        let result = DataURI::from_str("image/png;base64,abc");
        assert!(result.is_err());
    }

    #[test]
    fn parse_missing_separator_fails() {
        let result = DataURI::from_str("data:image/png,abc");
        assert!(result.is_err());
    }

    #[test]
    fn parse_empty_mime_fails() {
        let result = DataURI::from_str("data:;base64,YWJj");
        assert!(result.is_err());
    }

    #[test]
    fn parse_invalid_base64_fails() {
        let result = DataURI::from_str("data:text/plain;base64,!!invalid!!");
        assert!(result.is_err());
    }

    #[test]
    fn roundtrip_preserves_data() {
        let original = b"hello world";
        let uri = DataURI::new("text/plain".to_string(), original);
        let decoded = uri.decode().unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn roundtrip_with_binary_data() {
        let binary: Vec<u8> = (0..=255).collect();
        let uri = DataURI::new("application/octet-stream".to_string(), &binary);
        let decoded = uri.decode().unwrap();
        assert_eq!(decoded, binary);
    }

    #[test]
    fn display_format_is_correct() {
        let uri = DataURI::new("image/png".to_string(), b"test");
        let display = uri.to_string();
        assert!(display.starts_with("data:image/png;base64,"));
    }

    #[test]
    fn parse_then_display_roundtrip() {
        let input = "data:text/plain;base64,SGVsbG8gV29ybGQ=";
        let parsed = DataURI::from_str(input).unwrap();
        assert_eq!(parsed.to_string(), input);
    }
}
