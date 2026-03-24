use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use async_stream::stream;
use futures_core::stream::Stream;
use tokio::fs;

use crate::common::{RevisionToken, VaultPath};
use crate::storage::{File, FileContent, FileMeta, Storage, StorageError};

fn map_io_error(path: &VaultPath, err: std::io::Error) -> StorageError {
    match err.kind() {
        std::io::ErrorKind::NotFound => StorageError::NotFound(path.clone()),
        std::io::ErrorKind::PermissionDenied => StorageError::PermissionDenied(path.clone()),
        _ => StorageError::Io(path.clone(), err),
    }
}

/// Canonicalizes a path, resolving symlinks for the existing portion.
///
/// For paths where not all components exist yet (e.g., writing a new file),
/// canonicalizes the longest existing ancestor and appends the remaining
/// components. This ensures symlinks are resolved even for non-existent targets.
fn safe_canonicalize(path: &Path) -> std::io::Result<PathBuf> {
    // Try full canonicalization first (works when path exists)
    if let Ok(canonical) = path.canonicalize() {
        return Ok(canonical);
    }

    // Walk up until we find an existing ancestor
    let mut remaining = Vec::new();
    let mut current = path.to_path_buf();
    loop {
        if let Ok(canonical) = current.canonicalize() {
            // Rebuild the path: canonical ancestor + remaining components
            let mut result = canonical;
            for component in remaining.into_iter().rev() {
                result.push(component);
            }
            return Ok(result);
        }
        match current.file_name() {
            Some(name) => {
                remaining.push(name.to_os_string());
                current.pop();
            }
            None => return path.canonicalize(), // no existing ancestor, fail
        }
    }
}

fn revision_token(
    path: &VaultPath,
    metadata: &std::fs::Metadata,
) -> Result<RevisionToken, StorageError> {
    let modified = metadata
        .modified()
        .map_err(|e| StorageError::Io(path.clone(), e))?;
    let duration = modified
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| StorageError::InvalidData(path.clone(), e.to_string()))?;
    let file_size = metadata.len();
    Ok(format!("{}:{}", duration.as_nanos(), file_size).into())
}

/// Returns MIME type for known image extensions.
///
/// # Panics
/// Raise panics if called with an unrecognized extension. This function is only
/// called when `VaultPath::is_image()` returns true, which uses the same
/// extension list, making unknown extensions unreachable.
fn mime_from_extension(ext: &str) -> &'static str {
    match ext {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        "tiff" | "tif" => "image/tiff",
        _ => unreachable!("is_image() guards against unknown extensions"),
    }
}

pub struct LocalStorage {
    path: PathBuf,
    canonical_root: PathBuf,
    denied_paths: RwLock<HashSet<VaultPath>>,
    read_only_paths: RwLock<HashSet<VaultPath>>,
}

impl LocalStorage {
    pub fn new(path: PathBuf) -> std::io::Result<Self> {
        let canonical_root = path.canonicalize()?;
        Ok(LocalStorage {
            path,
            canonical_root,
            denied_paths: RwLock::new(HashSet::new()),
            read_only_paths: RwLock::new(HashSet::new()),
        })
    }

    fn is_denied(&self, path: &VaultPath) -> bool {
        self.denied_paths
            .read()
            .map(|guard| guard.contains(path))
            .unwrap_or(false)
    }

    fn is_read_only(&self, path: &VaultPath) -> bool {
        self.read_only_paths
            .read()
            .map(|guard| guard.contains(path))
            .unwrap_or(false)
    }

    fn resolve(&self, path: &VaultPath) -> Result<PathBuf, StorageError> {
        let full = self.path.join(path.as_str());
        let canonical = safe_canonicalize(&full).map_err(|e| map_io_error(path, e))?;
        if !canonical.starts_with(&self.canonical_root) {
            return Err(StorageError::PermissionDenied(path.clone()));
        }
        Ok(canonical)
    }

    async fn check_revision(
        &self,
        path: &VaultPath,
        expected: &RevisionToken,
    ) -> Result<(), StorageError> {
        let full_path = self.resolve(path)?;
        let metadata = fs::metadata(&full_path)
            .await
            .map_err(|e| map_io_error(path, e))?;
        let actual = revision_token(path, &metadata)?;
        if *expected != actual {
            return Err(StorageError::Conflict(
                path.clone(),
                expected.clone(),
                actual,
            ));
        }
        Ok(())
    }
}

impl Storage for LocalStorage {
    async fn list(&self) -> Result<impl Stream<Item = FileMeta>, StorageError> {
        let root = self.path.clone();
        Ok(stream! {
            let mut stack = vec![root.clone()];
            while let Some(dir) = stack.pop() {
                let mut entries = match fs::read_dir(&dir).await {
                    Ok(entries) => entries,
                    Err(_) => continue,
                };
                while let Ok(Some(entry)) = entries.next_entry().await {
                    let path = entry.path();
                    if path.is_dir() {
                        stack.push(path);
                    } else if let Ok(relative) = path.strip_prefix(&root)
                        && let Ok(vault_path) = VaultPath::try_from(relative)
                        && let Ok(metadata) = fs::metadata(&path).await
                    {
                        let size = metadata.len();
                        let modified = metadata.modified().unwrap_or(std::time::UNIX_EPOCH);
                        if let Ok(token) = revision_token(&vault_path, &metadata) {
                            yield FileMeta {
                                path: vault_path,
                                size,
                                modified,
                                revision_token: token,
                            };
                        }
                    }
                }
            }
        })
    }

    async fn read(&self, path: &VaultPath) -> Result<File, StorageError> {
        if self.is_denied(path) {
            return Err(StorageError::PermissionDenied(path.clone()));
        }

        let full_path = self.resolve(path)?;
        let metadata = fs::metadata(&full_path)
            .await
            .map_err(|e| map_io_error(path, e))?;

        let token = revision_token(path, &metadata)?;
        let size = metadata.len();
        let modified = metadata.modified().unwrap_or(std::time::UNIX_EPOCH);

        let meta = FileMeta {
            path: path.clone(),
            size,
            modified,
            revision_token: token,
        };

        let content = if path.is_image() {
            let ext = full_path.extension().and_then(|e| e.to_str()).unwrap_or("");
            let bytes = fs::read(&full_path)
                .await
                .map_err(|e| map_io_error(path, e))?;
            let mime = mime_from_extension(ext).to_string();
            FileContent::Image(crate::common::DataURI::new(mime, &bytes))
        } else {
            let content = fs::read_to_string(&full_path)
                .await
                .map_err(|e| map_io_error(path, e))?;
            FileContent::Markdown(content)
        };

        Ok(File { meta, content })
    }

    async fn write(
        &self,
        path: &VaultPath,
        data: FileContent,
        expected_token: Option<RevisionToken>,
    ) -> Result<RevisionToken, StorageError> {
        if self.is_denied(path) || self.is_read_only(path) {
            return Err(StorageError::PermissionDenied(path.clone()));
        }

        let full_path = self.resolve(path)?;

        if fs::try_exists(&full_path).await.unwrap_or(false)
            && let Some(expected) = &expected_token
        {
            self.check_revision(path, expected).await?;
        }

        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|e| map_io_error(path, e))?;
        }

        match data {
            FileContent::Markdown(content) => {
                fs::write(&full_path, content.as_bytes())
                    .await
                    .map_err(|e| map_io_error(path, e))?;
            }
            FileContent::Image(data_uri) => {
                let bytes = data_uri
                    .decode()
                    .map_err(|e| StorageError::InvalidData(path.clone(), e.to_string()))?;
                fs::write(&full_path, bytes)
                    .await
                    .map_err(|e| map_io_error(path, e))?;
            }
        }

        let metadata = fs::metadata(&full_path)
            .await
            .map_err(|e| map_io_error(path, e))?;
        revision_token(path, &metadata)
    }

    async fn delete(
        &self,
        path: &VaultPath,
        expected_token: RevisionToken,
    ) -> Result<(), StorageError> {
        if self.is_denied(path) || self.is_read_only(path) {
            return Err(StorageError::PermissionDenied(path.clone()));
        }

        self.check_revision(path, &expected_token).await?;

        let full_path = self.resolve(path)?;
        fs::remove_file(&full_path)
            .await
            .map_err(|e| map_io_error(path, e))
    }

    async fn r#move(
        &self,
        from: &VaultPath,
        to: &VaultPath,
        expected_token: RevisionToken,
    ) -> Result<(), StorageError> {
        if self.is_denied(from) || self.is_read_only(from) {
            return Err(StorageError::PermissionDenied(from.clone()));
        }
        if self.is_denied(to) || self.is_read_only(to) {
            return Err(StorageError::PermissionDenied(to.clone()));
        }

        self.check_revision(from, &expected_token).await?;

        let full_from = self.resolve(from)?;
        let full_to = self.resolve(to)?;

        if let Some(parent) = full_to.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|e| map_io_error(to, e))?;
        }

        fs::rename(&full_from, &full_to)
            .await
            .map_err(|e| map_io_error(from, e))
    }

    async fn copy(&self, from: &VaultPath, to: &VaultPath) -> Result<RevisionToken, StorageError> {
        if self.is_denied(from) {
            return Err(StorageError::PermissionDenied(from.clone()));
        }
        if self.is_denied(to) || self.is_read_only(to) {
            return Err(StorageError::PermissionDenied(to.clone()));
        }

        let full_from = self.resolve(from)?;
        let full_to = self.resolve(to)?;

        if let Some(parent) = full_to.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|e| map_io_error(to, e))?;
        }

        fs::copy(&full_from, &full_to)
            .await
            .map_err(|e| map_io_error(from, e))?;

        let metadata = fs::metadata(&full_to)
            .await
            .map_err(|e| map_io_error(to, e))?;
        revision_token(to, &metadata)
    }

    async fn exists(&self, path: &VaultPath) -> Result<bool, StorageError> {
        let full_path = self.resolve(path)?;
        fs::try_exists(&full_path)
            .await
            .map_err(|e| map_io_error(path, e))
    }

    fn deny_access(&self, paths: &[VaultPath]) {
        if let Ok(mut guard) = self.denied_paths.write() {
            *guard = paths.iter().cloned().collect();
        }
    }

    fn read_only_access(&self, paths: &[VaultPath]) {
        if let Ok(mut guard) = self.read_only_paths.write() {
            *guard = paths.iter().cloned().collect();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn safe_canonicalize_existing_path() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("note.md");
        std::fs::write(&file, "content").unwrap();

        let result = safe_canonicalize(&file).unwrap();
        assert_eq!(result, file.canonicalize().unwrap());
    }

    #[test]
    fn safe_canonicalize_nonexistent_file_in_existing_dir() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("new_file.md");

        let result = safe_canonicalize(&file).unwrap();
        let expected = dir.path().canonicalize().unwrap().join("new_file.md");
        assert_eq!(result, expected);
    }

    #[test]
    fn safe_canonicalize_nonexistent_nested_path() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("a/b/c.md");

        let result = safe_canonicalize(&file).unwrap();
        let expected = dir.path().canonicalize().unwrap().join("a/b/c.md");
        assert_eq!(result, expected);
    }

    #[cfg(unix)]
    #[test]
    fn safe_canonicalize_resolves_symlinks() {
        let dir = TempDir::new().unwrap();
        let real_dir = dir.path().join("real");
        std::fs::create_dir(&real_dir).unwrap();
        std::fs::write(real_dir.join("file.md"), "content").unwrap();

        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&real_dir, &link).unwrap();

        let result = safe_canonicalize(&link.join("file.md")).unwrap();
        assert_eq!(result, real_dir.canonicalize().unwrap().join("file.md"));
    }

    #[test]
    fn resolve_allows_valid_paths() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("note.md"), "content").unwrap();
        let storage = LocalStorage::new(dir.path().to_path_buf()).unwrap();

        let path = VaultPath::new("note.md").unwrap();
        assert!(storage.resolve(&path).is_ok());
    }

    #[test]
    fn resolve_allows_nonexistent_file_in_existing_dir() {
        let dir = TempDir::new().unwrap();
        let storage = LocalStorage::new(dir.path().to_path_buf()).unwrap();

        let path = VaultPath::new("new_note.md").unwrap();
        assert!(storage.resolve(&path).is_ok());
    }
}
