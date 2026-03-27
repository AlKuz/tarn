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
    let storage = LocalStorage::new(dir.path().to_path_buf()).unwrap();
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
            .write(&path, FileContent::Markdown("v1".to_string()), None)
            .await
            .unwrap();

        // Modify file externally to change its mtime
        tokio::time::sleep(Duration::from_millis(10)).await;
        fs::write(dir.path().join(path.as_str()), "external change")
            .await
            .unwrap();

        // Try to write with old token
        let result = storage
            .write(&path, FileContent::Markdown("v2".to_string()), Some(token1))
            .await;

        assert!(matches!(result, Err(StorageError::Conflict(_, _, _))));
    }

    #[tokio::test]
    async fn delete_with_stale_token_fails() {
        let (dir, storage) = create_temp_storage();
        let path = VaultPath::new("note.md").unwrap();

        let token1 = storage
            .write(&path, FileContent::Markdown("content".to_string()), None)
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
            .write(&from, FileContent::Markdown("content".to_string()), None)
            .await
            .unwrap();

        tokio::time::sleep(Duration::from_millis(10)).await;
        fs::write(dir.path().join(from.as_str()), "changed")
            .await
            .unwrap();

        let result = storage.r#move(&from, &to, token1).await;

        assert!(matches!(result, Err(StorageError::Conflict(_, _, _))));
    }
}

// =============================================================================
// Delete and Move Operations
// =============================================================================

mod delete_and_move {
    use super::*;

    #[tokio::test]
    async fn delete_removes_file_successfully() {
        let (_dir, storage) = create_temp_storage();
        let path = VaultPath::new("to_delete.md").unwrap();

        let token = storage
            .write(&path, FileContent::Markdown("content".to_string()), None)
            .await
            .unwrap();

        assert!(storage.exists(&path).await.unwrap());

        storage.delete(&path, token).await.unwrap();

        assert!(!storage.exists(&path).await.unwrap());
    }

    #[tokio::test]
    async fn move_renames_file_successfully() {
        let (_dir, storage) = create_temp_storage();
        let from = VaultPath::new("original.md").unwrap();
        let to = VaultPath::new("renamed.md").unwrap();

        let token = storage
            .write(&from, FileContent::Markdown("content".to_string()), None)
            .await
            .unwrap();

        storage.r#move(&from, &to, token).await.unwrap();

        assert!(!storage.exists(&from).await.unwrap());
        assert!(storage.exists(&to).await.unwrap());

        // Verify content preserved
        let file = storage.read(&to).await.unwrap();
        match file.content {
            FileContent::Markdown(content) => assert_eq!(content, "content"),
            _ => panic!("expected markdown"),
        }
    }

    #[tokio::test]
    async fn move_nonexistent_file_returns_not_found() {
        let (_dir, storage) = create_temp_storage();
        let from = VaultPath::new("missing.md").unwrap();
        let to = VaultPath::new("target.md").unwrap();

        let result = storage.r#move(&from, &to, "fake_token".into()).await;

        assert!(matches!(result, Err(StorageError::NotFound(_))));
    }

    #[tokio::test]
    async fn copy_preserves_source_file() {
        let (_dir, storage) = create_temp_storage();
        let from = VaultPath::new("source.md").unwrap();
        let to = VaultPath::new("copy.md").unwrap();

        storage
            .write(&from, FileContent::Markdown("original".to_string()), None)
            .await
            .unwrap();

        storage.copy(&from, &to).await.unwrap();

        // Both files should exist
        assert!(storage.exists(&from).await.unwrap());
        assert!(storage.exists(&to).await.unwrap());

        // Content should be identical
        let source = storage.read(&from).await.unwrap();
        let copy = storage.read(&to).await.unwrap();

        match (&source.content, &copy.content) {
            (FileContent::Markdown(s), FileContent::Markdown(c)) => assert_eq!(s, c),
            _ => panic!("expected markdown"),
        }
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
                FileContent::Markdown("deep content".to_string()),
                None,
            )
            .await;

        assert!(result.is_ok());
        assert!(storage.exists(&path).await.unwrap());
    }

    #[tokio::test]
    async fn rename_creates_target_parents() {
        let (_dir, storage) = create_temp_storage();
        let from = VaultPath::new("source.md").unwrap();
        let to = VaultPath::new("nested/target.md").unwrap();

        let token = storage
            .write(&from, FileContent::Markdown("content".to_string()), None)
            .await
            .unwrap();

        storage.r#move(&from, &to, token).await.unwrap();

        assert!(!storage.exists(&from).await.unwrap());
        assert!(storage.exists(&to).await.unwrap());
    }

    #[tokio::test]
    async fn copy_creates_target_parents() {
        let (_dir, storage) = create_temp_storage();
        let from = VaultPath::new("source.md").unwrap();
        let to = VaultPath::new("deep/nested/copy.md").unwrap();

        storage
            .write(&from, FileContent::Markdown("content".to_string()), None)
            .await
            .unwrap();

        let result = storage.copy(&from, &to).await;

        assert!(result.is_ok());
        assert!(storage.exists(&from).await.unwrap());
        assert!(storage.exists(&to).await.unwrap());
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

        let file = storage.read(&path).await.unwrap();

        assert!(matches!(file.content, FileContent::Markdown(_)));
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

        let file = storage.read(&path).await.unwrap();

        match file.content {
            FileContent::Image(uri) => {
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

        let file = storage.read(&path).await.unwrap();

        assert!(matches!(file.content, FileContent::Markdown(_)));
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
    async fn exists_missing_returns_false_not_error() {
        let (_dir, storage) = create_temp_storage();
        let path = VaultPath::new("nonexistent.md").unwrap();

        let exists = storage.exists(&path).await.unwrap();

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
        let mut files: Vec<_> = stream.map(|m| m.path).collect().await;
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
        let files: Vec<_> = stream.map(|m| m.path).collect().await;

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

// =============================================================================
// Access Control (deny_access / read_only_access)
// =============================================================================

mod access_control {
    use super::*;

    #[tokio::test]
    async fn deny_access_blocks_read() {
        let (dir, storage) = create_temp_storage();
        let path = VaultPath::new("secret.md").unwrap();

        fs::write(dir.path().join(path.as_str()), "secret content")
            .await
            .unwrap();

        storage.deny_access(std::slice::from_ref(&path));

        let result = storage.read(&path).await;

        assert!(matches!(result, Err(StorageError::PermissionDenied(_))));
    }

    #[tokio::test]
    async fn deny_access_blocks_write() {
        let (_dir, storage) = create_temp_storage();
        let path = VaultPath::new("protected.md").unwrap();

        storage.deny_access(std::slice::from_ref(&path));

        let result = storage
            .write(&path, FileContent::Markdown("test".to_string()), None)
            .await;

        assert!(matches!(result, Err(StorageError::PermissionDenied(_))));
    }

    #[tokio::test]
    async fn deny_access_blocks_delete() {
        let (_dir, storage) = create_temp_storage();
        let path = VaultPath::new("note.md").unwrap();

        let token = storage
            .write(&path, FileContent::Markdown("content".to_string()), None)
            .await
            .unwrap();

        storage.deny_access(std::slice::from_ref(&path));

        let result = storage.delete(&path, token).await;

        assert!(matches!(result, Err(StorageError::PermissionDenied(_))));
    }

    #[tokio::test]
    async fn deny_access_blocks_move_from() {
        let (_dir, storage) = create_temp_storage();
        let from = VaultPath::new("source.md").unwrap();
        let to = VaultPath::new("target.md").unwrap();

        let token = storage
            .write(&from, FileContent::Markdown("content".to_string()), None)
            .await
            .unwrap();

        storage.deny_access(std::slice::from_ref(&from));

        let result = storage.r#move(&from, &to, token).await;

        assert!(matches!(result, Err(StorageError::PermissionDenied(_))));
    }

    #[tokio::test]
    async fn deny_access_blocks_move_to() {
        let (_dir, storage) = create_temp_storage();
        let from = VaultPath::new("source.md").unwrap();
        let to = VaultPath::new("target.md").unwrap();

        let token = storage
            .write(&from, FileContent::Markdown("content".to_string()), None)
            .await
            .unwrap();

        storage.deny_access(std::slice::from_ref(&to));

        let result = storage.r#move(&from, &to, token).await;

        assert!(matches!(result, Err(StorageError::PermissionDenied(_))));
    }

    #[tokio::test]
    async fn deny_access_blocks_copy_from() {
        let (dir, storage) = create_temp_storage();
        let from = VaultPath::new("source.md").unwrap();
        let to = VaultPath::new("target.md").unwrap();

        fs::write(dir.path().join(from.as_str()), "content")
            .await
            .unwrap();

        storage.deny_access(std::slice::from_ref(&from));

        let result = storage.copy(&from, &to).await;

        assert!(matches!(result, Err(StorageError::PermissionDenied(_))));
    }

    #[tokio::test]
    async fn deny_access_blocks_copy_to() {
        let (dir, storage) = create_temp_storage();
        let from = VaultPath::new("source.md").unwrap();
        let to = VaultPath::new("target.md").unwrap();

        fs::write(dir.path().join(from.as_str()), "content")
            .await
            .unwrap();

        storage.deny_access(std::slice::from_ref(&to));

        let result = storage.copy(&from, &to).await;

        assert!(matches!(result, Err(StorageError::PermissionDenied(_))));
    }

    #[tokio::test]
    async fn read_only_allows_read_but_blocks_write() {
        let (dir, storage) = create_temp_storage();
        let path = VaultPath::new("readonly.md").unwrap();

        fs::write(dir.path().join(path.as_str()), "original")
            .await
            .unwrap();

        storage.read_only_access(std::slice::from_ref(&path));

        // Read should succeed
        let file = storage.read(&path).await.unwrap();
        assert!(matches!(file.content, FileContent::Markdown(_)));

        // Write should fail
        let result = storage
            .write(&path, FileContent::Markdown("modified".to_string()), None)
            .await;
        assert!(matches!(result, Err(StorageError::PermissionDenied(_))));
    }

    #[tokio::test]
    async fn read_only_blocks_delete() {
        let (_dir, storage) = create_temp_storage();
        let path = VaultPath::new("note.md").unwrap();

        let token = storage
            .write(&path, FileContent::Markdown("content".to_string()), None)
            .await
            .unwrap();

        storage.read_only_access(std::slice::from_ref(&path));

        let result = storage.delete(&path, token).await;

        assert!(matches!(result, Err(StorageError::PermissionDenied(_))));
    }

    #[tokio::test]
    async fn read_only_blocks_move_from() {
        let (_dir, storage) = create_temp_storage();
        let from = VaultPath::new("source.md").unwrap();
        let to = VaultPath::new("target.md").unwrap();

        let token = storage
            .write(&from, FileContent::Markdown("content".to_string()), None)
            .await
            .unwrap();

        storage.read_only_access(std::slice::from_ref(&from));

        let result = storage.r#move(&from, &to, token).await;

        assert!(matches!(result, Err(StorageError::PermissionDenied(_))));
    }

    #[tokio::test]
    async fn read_only_blocks_move_to() {
        let (_dir, storage) = create_temp_storage();
        let from = VaultPath::new("source.md").unwrap();
        let to = VaultPath::new("target.md").unwrap();

        let token = storage
            .write(&from, FileContent::Markdown("content".to_string()), None)
            .await
            .unwrap();

        storage.read_only_access(std::slice::from_ref(&to));

        let result = storage.r#move(&from, &to, token).await;

        assert!(matches!(result, Err(StorageError::PermissionDenied(_))));
    }

    #[tokio::test]
    async fn read_only_blocks_copy_to() {
        let (dir, storage) = create_temp_storage();
        let from = VaultPath::new("source.md").unwrap();
        let to = VaultPath::new("target.md").unwrap();

        fs::write(dir.path().join(from.as_str()), "content")
            .await
            .unwrap();

        storage.read_only_access(std::slice::from_ref(&to));

        let result = storage.copy(&from, &to).await;

        assert!(matches!(result, Err(StorageError::PermissionDenied(_))));
    }

    #[tokio::test]
    async fn deny_access_can_be_updated() {
        let (dir, storage) = create_temp_storage();
        let path1 = VaultPath::new("note1.md").unwrap();
        let path2 = VaultPath::new("note2.md").unwrap();

        fs::write(dir.path().join(path1.as_str()), "content1")
            .await
            .unwrap();
        fs::write(dir.path().join(path2.as_str()), "content2")
            .await
            .unwrap();

        // Deny path1
        storage.deny_access(std::slice::from_ref(&path1));
        assert!(storage.read(&path1).await.is_err());
        assert!(storage.read(&path2).await.is_ok());

        // Update to deny path2 instead
        storage.deny_access(std::slice::from_ref(&path2));
        assert!(storage.read(&path1).await.is_ok());
        assert!(storage.read(&path2).await.is_err());

        // Clear all denials
        storage.deny_access(&[]);
        assert!(storage.read(&path1).await.is_ok());
        assert!(storage.read(&path2).await.is_ok());
    }
}

// =============================================================================
// Image Writing
// =============================================================================

mod image_write {
    use super::*;
    use tarn::common::DataURI;

    #[tokio::test]
    async fn write_image_as_data_uri() {
        let (_dir, storage) = create_temp_storage();
        let path = VaultPath::new("image.png").unwrap();

        // Minimal PNG bytes
        let png_bytes: [u8; 67] = [
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48,
            0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00,
            0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78,
            0x9C, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00,
            0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
        ];

        let data_uri = DataURI::new("image/png".to_string(), &png_bytes);
        let result = storage
            .write(&path, FileContent::Image(data_uri), None)
            .await;

        assert!(result.is_ok());

        // Verify written content matches
        let file = storage.read(&path).await.unwrap();
        match file.content {
            FileContent::Image(uri) => {
                let decoded = uri.decode().unwrap();
                assert_eq!(decoded, png_bytes);
            }
            _ => panic!("expected Image content"),
        }
    }

    #[tokio::test]
    async fn write_image_creates_parent_directories() {
        let (_dir, storage) = create_temp_storage();
        let path = VaultPath::new("assets/images/deep/photo.jpg").unwrap();

        let jpeg_marker = b"\xFF\xD8\xFF"; // JPEG magic bytes
        let data_uri = DataURI::new("image/jpeg".to_string(), jpeg_marker);

        let result = storage
            .write(&path, FileContent::Image(data_uri), None)
            .await;

        assert!(result.is_ok());
        assert!(storage.exists(&path).await.unwrap());
    }

    #[tokio::test]
    async fn write_image_with_revision_conflict() {
        let (dir, storage) = create_temp_storage();
        let path = VaultPath::new("image.png").unwrap();

        let png_bytes = [0x89, 0x50, 0x4E, 0x47];
        let data_uri = DataURI::new("image/png".to_string(), &png_bytes);

        let token1 = storage
            .write(&path, FileContent::Image(data_uri.clone()), None)
            .await
            .unwrap();

        // External modification
        tokio::time::sleep(Duration::from_millis(10)).await;
        fs::write(dir.path().join(path.as_str()), b"modified")
            .await
            .unwrap();

        // Attempt to write with stale token
        let result = storage
            .write(&path, FileContent::Image(data_uri), Some(token1))
            .await;

        assert!(matches!(result, Err(StorageError::Conflict(_, _, _))));
    }
}

// =============================================================================
// MIME Type Detection (additional formats)
// =============================================================================

mod mime_types {
    use super::*;

    #[tokio::test]
    async fn jpeg_read_as_image() {
        let (dir, storage) = create_temp_storage();
        let path = VaultPath::new("photo.jpg").unwrap();

        // JPEG magic bytes (SOI marker)
        let jpeg_bytes = [0xFF, 0xD8, 0xFF, 0xE0];
        fs::write(dir.path().join(path.as_str()), &jpeg_bytes)
            .await
            .unwrap();

        let file = storage.read(&path).await.unwrap();

        assert!(matches!(file.content, FileContent::Image(_)));
    }

    #[tokio::test]
    async fn jpeg_extension_read_as_image() {
        let (dir, storage) = create_temp_storage();
        let path = VaultPath::new("photo.jpeg").unwrap();

        let jpeg_bytes = [0xFF, 0xD8, 0xFF, 0xE0];
        fs::write(dir.path().join(path.as_str()), &jpeg_bytes)
            .await
            .unwrap();

        let file = storage.read(&path).await.unwrap();

        assert!(matches!(file.content, FileContent::Image(_)));
    }

    #[tokio::test]
    async fn gif_read_as_image() {
        let (dir, storage) = create_temp_storage();
        let path = VaultPath::new("animation.gif").unwrap();

        // GIF magic bytes
        let gif_bytes = b"GIF89a";
        fs::write(dir.path().join(path.as_str()), gif_bytes)
            .await
            .unwrap();

        let file = storage.read(&path).await.unwrap();

        assert!(matches!(file.content, FileContent::Image(_)));
    }

    #[tokio::test]
    async fn webp_read_as_image() {
        let (dir, storage) = create_temp_storage();
        let path = VaultPath::new("modern.webp").unwrap();

        // WebP magic bytes (RIFF....WEBP)
        let webp_bytes = b"RIFF\x00\x00\x00\x00WEBP";
        fs::write(dir.path().join(path.as_str()), webp_bytes)
            .await
            .unwrap();

        let file = storage.read(&path).await.unwrap();

        assert!(matches!(file.content, FileContent::Image(_)));
    }

    #[tokio::test]
    async fn svg_read_as_image() {
        let (dir, storage) = create_temp_storage();
        let path = VaultPath::new("icon.svg").unwrap();

        let svg_content = r#"<svg xmlns="http://www.w3.org/2000/svg"></svg>"#;
        fs::write(dir.path().join(path.as_str()), svg_content)
            .await
            .unwrap();

        let file = storage.read(&path).await.unwrap();

        assert!(matches!(file.content, FileContent::Image(_)));
    }

    #[tokio::test]
    async fn bmp_read_as_image() {
        let (dir, storage) = create_temp_storage();
        let path = VaultPath::new("bitmap.bmp").unwrap();

        // BMP magic bytes
        let bmp_bytes = b"BM";
        fs::write(dir.path().join(path.as_str()), bmp_bytes)
            .await
            .unwrap();

        let file = storage.read(&path).await.unwrap();

        assert!(matches!(file.content, FileContent::Image(_)));
    }

    #[tokio::test]
    async fn ico_read_as_image() {
        let (dir, storage) = create_temp_storage();
        let path = VaultPath::new("favicon.ico").unwrap();

        // ICO magic bytes
        let ico_bytes = [0x00, 0x00, 0x01, 0x00];
        fs::write(dir.path().join(path.as_str()), &ico_bytes)
            .await
            .unwrap();

        let file = storage.read(&path).await.unwrap();

        assert!(matches!(file.content, FileContent::Image(_)));
    }

    #[tokio::test]
    async fn tiff_read_as_image() {
        let (dir, storage) = create_temp_storage();
        let path = VaultPath::new("document.tiff").unwrap();

        // TIFF magic bytes (little-endian)
        let tiff_bytes = b"II\x2A\x00";
        fs::write(dir.path().join(path.as_str()), tiff_bytes)
            .await
            .unwrap();

        let file = storage.read(&path).await.unwrap();

        assert!(matches!(file.content, FileContent::Image(_)));
    }

    #[tokio::test]
    async fn tif_extension_read_as_image() {
        let (dir, storage) = create_temp_storage();
        let path = VaultPath::new("scan.tif").unwrap();

        let tiff_bytes = b"MM\x00\x2A"; // big-endian TIFF
        fs::write(dir.path().join(path.as_str()), tiff_bytes)
            .await
            .unwrap();

        let file = storage.read(&path).await.unwrap();

        assert!(matches!(file.content, FileContent::Image(_)));
    }
}

// =============================================================================
// Cross-Platform Line Endings (CRLF support for Windows)
// =============================================================================

mod crlf_support {
    use super::*;
    use tarn::TarnConfig;
    use tarn::common::Buildable;
    use tarn::note_handler::Note;

    #[tokio::test]
    async fn reads_file_with_crlf_line_endings() {
        let dir = TempDir::new().unwrap();

        // Write file with Windows-style CRLF line endings
        let content = "---\r\ntitle: Windows Note\r\ntags:\r\n  - windows\r\n  - testing\r\n---\r\n# Hello\r\n\r\nContent here.\r\n";
        fs::write(dir.path().join("note.md"), content)
            .await
            .unwrap();

        let storage = LocalStorage::new(dir.path().to_path_buf()).unwrap();
        let path = VaultPath::new("note.md").unwrap();

        let file = storage.read(&path).await.unwrap();

        match file.content {
            FileContent::Markdown(content) => {
                let note = Note::from(content.as_str());
                let fm = note.frontmatter.as_ref().expect("should have frontmatter");
                assert_eq!(fm.title, Some("Windows Note".to_string()));
                assert_eq!(fm.tags, vec!["windows", "testing"]);
            }
            _ => panic!("expected markdown"),
        }
    }

    #[tokio::test]
    async fn tarn_core_parses_crlf_notes() {
        let dir = TempDir::new().unwrap();

        // Write multiple files with CRLF
        fs::write(
            dir.path().join("rust.md"),
            "---\r\ntags:\r\n  - programming/rust\r\n---\r\n# Rust\r\n\r\nRust content.\r\n",
        )
        .await
        .unwrap();

        fs::write(
            dir.path().join("python.md"),
            "---\r\ntags:\r\n  - programming/python\r\n---\r\n# Python\r\n\r\nPython content.\r\n",
        )
        .await
        .unwrap();

        let core = TarnConfig::local(dir.path().to_path_buf()).build().unwrap();

        // Test vault_tags returns correctly parsed tags
        let tags_response = core.vault_tags(None).await.unwrap();
        let tag_names: Vec<&str> = tags_response.tags.iter().map(|t| t.tag.as_str()).collect();

        assert!(tag_names.contains(&"programming/rust"));
        assert!(tag_names.contains(&"programming/python"));
    }

    #[tokio::test]
    async fn search_works_with_crlf_content() {
        let dir = TempDir::new().unwrap();

        fs::write(
            dir.path().join("searchable.md"),
            "---\r\ntitle: Searchable\r\n---\r\n# Searchable\r\n\r\nThis note contains unique_term_123 for testing.\r\n",
        )
        .await
        .unwrap();

        let core = TarnConfig::local(dir.path().to_path_buf()).build().unwrap();

        let results = core
            .search_notes("unique_term_123", None, None, 10, 0)
            .await
            .unwrap();

        assert_eq!(results.total, 1);
        assert_eq!(results.results[0].title, Some("Searchable".to_string()));
    }
}

// =============================================================================
// Unix Permission Errors (platform-specific)
// =============================================================================

#[cfg(unix)]
mod unix_permissions {
    use std::os::unix::fs::PermissionsExt;

    use tempfile::TempDir;
    use tokio::fs;

    use tarn::common::VaultPath;
    use tarn::storage::{FileContent, LocalStorage, Storage, StorageError};

    use super::create_temp_storage;

    #[tokio::test]
    async fn read_permission_denied_returns_error() {
        let (dir, storage) = create_temp_storage();
        let path = VaultPath::new("secret.md").unwrap();

        let file_path = dir.path().join(path.as_str());
        fs::write(&file_path, "secret content").await.unwrap();

        // Remove all permissions
        let mut perms = std::fs::metadata(&file_path).unwrap().permissions();
        perms.set_mode(0o000);
        std::fs::set_permissions(&file_path, perms).unwrap();

        let result = storage.read(&path).await;

        // Restore permissions for cleanup
        let mut perms = std::fs::metadata(&file_path).unwrap().permissions();
        perms.set_mode(0o644);
        std::fs::set_permissions(&file_path, perms).unwrap();

        assert!(matches!(result, Err(StorageError::PermissionDenied(_))));
    }

    #[tokio::test]
    async fn write_to_readonly_parent_fails() {
        let dir = TempDir::new().unwrap();
        let storage = LocalStorage::new(dir.path().to_path_buf()).unwrap();

        // Create a readonly directory
        let readonly_dir = dir.path().join("readonly");
        std::fs::create_dir(&readonly_dir).unwrap();
        let mut perms = std::fs::metadata(&readonly_dir).unwrap().permissions();
        perms.set_mode(0o555); // r-xr-xr-x
        std::fs::set_permissions(&readonly_dir, perms).unwrap();

        let path = VaultPath::new("readonly/nested/note.md").unwrap();

        let result = storage
            .write(&path, FileContent::Markdown("content".to_string()), None)
            .await;

        // Restore permissions for cleanup
        let mut perms = std::fs::metadata(&readonly_dir).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&readonly_dir, perms).unwrap();

        assert!(matches!(result, Err(StorageError::PermissionDenied(_))));
    }

    #[tokio::test]
    async fn move_to_readonly_parent_fails() {
        let dir = TempDir::new().unwrap();
        let storage = LocalStorage::new(dir.path().to_path_buf()).unwrap();

        // Create source file
        let from = VaultPath::new("source.md").unwrap();
        let token = storage
            .write(&from, FileContent::Markdown("content".to_string()), None)
            .await
            .unwrap();

        // Create a readonly directory
        let readonly_dir = dir.path().join("readonly");
        std::fs::create_dir(&readonly_dir).unwrap();
        let mut perms = std::fs::metadata(&readonly_dir).unwrap().permissions();
        perms.set_mode(0o555);
        std::fs::set_permissions(&readonly_dir, perms).unwrap();

        let to = VaultPath::new("readonly/nested/target.md").unwrap();
        let result = storage.r#move(&from, &to, token).await;

        // Restore permissions for cleanup
        let mut perms = std::fs::metadata(&readonly_dir).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&readonly_dir, perms).unwrap();

        assert!(matches!(result, Err(StorageError::PermissionDenied(_))));
    }

    #[tokio::test]
    async fn copy_to_readonly_parent_fails() {
        let dir = TempDir::new().unwrap();
        let storage = LocalStorage::new(dir.path().to_path_buf()).unwrap();

        // Create source file
        let from = VaultPath::new("source.md").unwrap();
        storage
            .write(&from, FileContent::Markdown("content".to_string()), None)
            .await
            .unwrap();

        // Create a readonly directory
        let readonly_dir = dir.path().join("readonly");
        std::fs::create_dir(&readonly_dir).unwrap();
        let mut perms = std::fs::metadata(&readonly_dir).unwrap().permissions();
        perms.set_mode(0o555);
        std::fs::set_permissions(&readonly_dir, perms).unwrap();

        let to = VaultPath::new("readonly/nested/copy.md").unwrap();
        let result = storage.copy(&from, &to).await;

        // Restore permissions for cleanup
        let mut perms = std::fs::metadata(&readonly_dir).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&readonly_dir, perms).unwrap();

        assert!(matches!(result, Err(StorageError::PermissionDenied(_))));
    }
}

// =============================================================================
// Symlink Traversal Prevention (platform-specific)
// =============================================================================

#[cfg(unix)]
mod symlink_traversal {
    use tempfile::TempDir;
    use tokio::fs;

    use tarn::common::VaultPath;
    use tarn::storage::{FileContent, LocalStorage, Storage, StorageError};

    #[tokio::test]
    async fn read_through_symlink_outside_vault_is_denied() {
        let vault_dir = TempDir::new().unwrap();
        let outside_dir = TempDir::new().unwrap();

        // Create a file outside the vault
        fs::write(outside_dir.path().join("secret.md"), "secret content")
            .await
            .unwrap();

        // Create a symlink inside the vault pointing outside
        let link_path = vault_dir.path().join("escape");
        std::os::unix::fs::symlink(outside_dir.path(), &link_path).unwrap();

        let storage = LocalStorage::new(vault_dir.path().to_path_buf()).unwrap();
        let path = VaultPath::new("escape/secret.md").unwrap();

        let result = storage.read(&path).await;
        assert!(
            matches!(result, Err(StorageError::PermissionDenied(_))),
            "expected PermissionDenied, got: {result:?}"
        );
    }

    #[tokio::test]
    async fn write_through_symlink_outside_vault_is_denied() {
        let vault_dir = TempDir::new().unwrap();
        let outside_dir = TempDir::new().unwrap();

        // Create a symlink inside the vault pointing outside
        let link_path = vault_dir.path().join("escape");
        std::os::unix::fs::symlink(outside_dir.path(), &link_path).unwrap();

        let storage = LocalStorage::new(vault_dir.path().to_path_buf()).unwrap();
        let path = VaultPath::new("escape/malicious.md").unwrap();

        let result = storage
            .write(&path, FileContent::Markdown("pwned".into()), None)
            .await;
        assert!(
            matches!(result, Err(StorageError::PermissionDenied(_))),
            "expected PermissionDenied, got: {result:?}"
        );

        // Verify the file was NOT created outside the vault
        assert!(!outside_dir.path().join("malicious.md").exists());
    }

    #[tokio::test]
    async fn symlink_within_vault_is_allowed() {
        let vault_dir = TempDir::new().unwrap();

        // Create a real directory and file inside the vault
        let real_dir = vault_dir.path().join("real");
        fs::create_dir(&real_dir).await.unwrap();
        fs::write(real_dir.join("note.md"), "# Hello\n\nContent here.")
            .await
            .unwrap();

        // Create a symlink inside the vault pointing to another location inside the vault
        let link_path = vault_dir.path().join("alias");
        std::os::unix::fs::symlink(&real_dir, &link_path).unwrap();

        let storage = LocalStorage::new(vault_dir.path().to_path_buf()).unwrap();
        let path = VaultPath::new("alias/note.md").unwrap();

        let result = storage.read(&path).await;
        assert!(result.is_ok(), "expected Ok, got: {result:?}");
    }

    #[tokio::test]
    async fn nested_symlink_escape_is_denied() {
        let vault_dir = TempDir::new().unwrap();
        let outside_dir = TempDir::new().unwrap();

        fs::write(outside_dir.path().join("secret.md"), "secret")
            .await
            .unwrap();

        // Create nested directory structure with symlink escape
        let nested = vault_dir.path().join("a/b");
        fs::create_dir_all(&nested).await.unwrap();
        let link_path = nested.join("escape");
        std::os::unix::fs::symlink(outside_dir.path(), &link_path).unwrap();

        let storage = LocalStorage::new(vault_dir.path().to_path_buf()).unwrap();
        let path = VaultPath::new("a/b/escape/secret.md").unwrap();

        let result = storage.read(&path).await;
        assert!(
            matches!(result, Err(StorageError::PermissionDenied(_))),
            "expected PermissionDenied, got: {result:?}"
        );
    }

    #[tokio::test]
    async fn symlink_file_outside_vault_is_denied() {
        let vault_dir = TempDir::new().unwrap();
        let outside_dir = TempDir::new().unwrap();

        let outside_file = outside_dir.path().join("secret.md");
        fs::write(&outside_file, "secret content").await.unwrap();

        // Create a symlink to a single file outside the vault
        let link_path = vault_dir.path().join("linked.md");
        std::os::unix::fs::symlink(&outside_file, &link_path).unwrap();

        let storage = LocalStorage::new(vault_dir.path().to_path_buf()).unwrap();
        let path = VaultPath::new("linked.md").unwrap();

        let result = storage.read(&path).await;
        assert!(
            matches!(result, Err(StorageError::PermissionDenied(_))),
            "expected PermissionDenied, got: {result:?}"
        );
    }
}

// =============================================================================
// Symlink Traversal Prevention (Windows)
// =============================================================================

#[cfg(windows)]
mod symlink_traversal {
    use tempfile::TempDir;
    use tokio::fs;

    use tarn::common::VaultPath;
    use tarn::storage::{FileContent, LocalStorage, Storage, StorageError};

    #[tokio::test]
    async fn read_through_dir_symlink_outside_vault_is_denied() {
        let vault_dir = TempDir::new().unwrap();
        let outside_dir = TempDir::new().unwrap();

        fs::write(outside_dir.path().join("secret.md"), "secret content")
            .await
            .unwrap();

        let link_path = vault_dir.path().join("escape");
        std::os::windows::fs::symlink_dir(outside_dir.path(), &link_path).unwrap();

        let storage = LocalStorage::new(vault_dir.path().to_path_buf()).unwrap();
        let path = VaultPath::new("escape/secret.md").unwrap();

        let result = storage.read(&path).await;
        assert!(
            matches!(result, Err(StorageError::PermissionDenied(_))),
            "expected PermissionDenied, got: {result:?}"
        );
    }

    #[tokio::test]
    async fn write_through_dir_symlink_outside_vault_is_denied() {
        let vault_dir = TempDir::new().unwrap();
        let outside_dir = TempDir::new().unwrap();

        let link_path = vault_dir.path().join("escape");
        std::os::windows::fs::symlink_dir(outside_dir.path(), &link_path).unwrap();

        let storage = LocalStorage::new(vault_dir.path().to_path_buf()).unwrap();
        let path = VaultPath::new("escape/malicious.md").unwrap();

        let result = storage
            .write(&path, FileContent::Markdown("pwned".into()), None)
            .await;
        assert!(
            matches!(result, Err(StorageError::PermissionDenied(_))),
            "expected PermissionDenied, got: {result:?}"
        );

        assert!(!outside_dir.path().join("malicious.md").exists());
    }

    #[tokio::test]
    async fn dir_symlink_within_vault_is_allowed() {
        let vault_dir = TempDir::new().unwrap();

        let real_dir = vault_dir.path().join("real");
        fs::create_dir(&real_dir).await.unwrap();
        fs::write(real_dir.join("note.md"), "# Hello\n\nContent here.")
            .await
            .unwrap();

        let link_path = vault_dir.path().join("alias");
        std::os::windows::fs::symlink_dir(&real_dir, &link_path).unwrap();

        let storage = LocalStorage::new(vault_dir.path().to_path_buf()).unwrap();
        let path = VaultPath::new("alias/note.md").unwrap();

        let result = storage.read(&path).await;
        assert!(result.is_ok(), "expected Ok, got: {result:?}");
    }

    #[tokio::test]
    async fn nested_dir_symlink_escape_is_denied() {
        let vault_dir = TempDir::new().unwrap();
        let outside_dir = TempDir::new().unwrap();

        fs::write(outside_dir.path().join("secret.md"), "secret")
            .await
            .unwrap();

        let nested = vault_dir.path().join("a\\b");
        fs::create_dir_all(&nested).await.unwrap();
        let link_path = nested.join("escape");
        std::os::windows::fs::symlink_dir(outside_dir.path(), &link_path).unwrap();

        let storage = LocalStorage::new(vault_dir.path().to_path_buf()).unwrap();
        let path = VaultPath::new("a/b/escape/secret.md").unwrap();

        let result = storage.read(&path).await;
        assert!(
            matches!(result, Err(StorageError::PermissionDenied(_))),
            "expected PermissionDenied, got: {result:?}"
        );
    }

    #[tokio::test]
    async fn file_symlink_outside_vault_is_denied() {
        let vault_dir = TempDir::new().unwrap();
        let outside_dir = TempDir::new().unwrap();

        let outside_file = outside_dir.path().join("secret.md");
        fs::write(&outside_file, "secret content").await.unwrap();

        let link_path = vault_dir.path().join("linked.md");
        std::os::windows::fs::symlink_file(&outside_file, &link_path).unwrap();

        let storage = LocalStorage::new(vault_dir.path().to_path_buf()).unwrap();
        let path = VaultPath::new("linked.md").unwrap();

        let result = storage.read(&path).await;
        assert!(
            matches!(result, Err(StorageError::PermissionDenied(_))),
            "expected PermissionDenied, got: {result:?}"
        );
    }
}
