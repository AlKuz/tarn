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

fn is_path_safe(root: &Path, resolved: &Path) -> bool {
    resolved.starts_with(root)
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
        _ => "application/octet-stream",
    }
}

pub struct LocalStorage {
    path: PathBuf,
    denied_paths: RwLock<HashSet<VaultPath>>,
    read_only_paths: RwLock<HashSet<VaultPath>>,
}

impl LocalStorage {
    pub fn new(path: PathBuf) -> Self {
        LocalStorage {
            path,
            denied_paths: RwLock::new(HashSet::new()),
            read_only_paths: RwLock::new(HashSet::new()),
        }
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
        if !is_path_safe(&self.path, &full) {
            return Err(StorageError::PermissionDenied(path.clone()));
        }
        Ok(full)
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
