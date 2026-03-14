use std::path::{Path, PathBuf};

use async_stream::stream;
use futures_core::stream::Stream;
use tokio::fs;

use crate::common::{RevisionToken, VaultPath};
use crate::storage::{FileContent, Storage, StorageError};

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
}

impl LocalStorage {
    pub fn new(path: PathBuf) -> Self {
        LocalStorage { path }
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
    async fn list(&self) -> Result<impl Stream<Item = VaultPath>, StorageError> {
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
                    {
                        yield vault_path;
                    }
                }
            }
        })
    }

    async fn read(&self, path: &VaultPath) -> Result<FileContent, StorageError> {
        let full_path = self.resolve(path)?;
        let metadata = fs::metadata(&full_path)
            .await
            .map_err(|e| map_io_error(path, e))?;

        let token = revision_token(path, &metadata)?;

        if path.is_image() {
            let ext = full_path.extension().and_then(|e| e.to_str()).unwrap_or("");
            let bytes = fs::read(&full_path)
                .await
                .map_err(|e| map_io_error(path, e))?;
            let mime = mime_from_extension(ext).to_string();
            Ok(FileContent::Image {
                content: crate::common::DataURI::new(mime, &bytes),
                token,
            })
        } else {
            let content = fs::read_to_string(&full_path)
                .await
                .map_err(|e| map_io_error(path, e))?;
            Ok(FileContent::Markdown { content, token })
        }
    }

    async fn write(&self, path: &VaultPath, data: FileContent) -> Result<RevisionToken, StorageError> {
        let full_path = self.resolve(path)?;

        if fs::try_exists(&full_path).await.unwrap_or(false) {
            let expected = match &data {
                FileContent::Markdown { token, .. } => token,
                FileContent::Image { token, .. } => token,
            };
            self.check_revision(path, expected).await?;
        }

        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|e| map_io_error(path, e))?;
        }

        match data {
            FileContent::Markdown { content, .. } => {
                fs::write(&full_path, content.as_bytes())
                    .await
                    .map_err(|e| map_io_error(path, e))?;
            }
            FileContent::Image {
                content: data_uri, ..
            } => {
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
        self.check_revision(path, &expected_token).await?;

        let full_path = self.resolve(path)?;
        fs::remove_file(&full_path)
            .await
            .map_err(|e| map_io_error(path, e))
    }

    async fn rename(
        &self,
        from: &VaultPath,
        to: &VaultPath,
        expected_token: RevisionToken,
    ) -> Result<(), StorageError> {
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

    async fn is_exists(&self, path: &VaultPath) -> Result<bool, StorageError> {
        let full_path = self.resolve(path)?;
        fs::try_exists(&full_path)
            .await
            .map_err(|e| map_io_error(path, e))
    }
}
