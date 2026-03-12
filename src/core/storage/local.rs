use std::path::{Path, PathBuf};

use async_stream::stream;
use futures_core::stream::Stream;
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::fs;
use tokio::sync::mpsc;

use crate::core::common::RevisionToken;
use crate::core::storage::{FileContent, Storage, StorageError, StorageEvent, StorageEventListener};

fn map_io_error(path: &Path, err: std::io::Error) -> StorageError {
    match err.kind() {
        std::io::ErrorKind::NotFound => StorageError::NotFound(path.to_path_buf()),
        std::io::ErrorKind::PermissionDenied => StorageError::PermissionDenied(path.to_path_buf()),
        _ => StorageError::Io(path.to_path_buf(), err),
    }
}

fn revision_token(path: &Path, metadata: &std::fs::Metadata) -> Result<RevisionToken, StorageError> {
    let modified = metadata
        .modified()
        .map_err(|e| StorageError::Io(path.to_path_buf(), e))?;
    let duration = modified
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| StorageError::InvalidData(path.to_path_buf(), e.to_string()))?;
    let file_size = metadata.len();
    Ok(format!("{}:{}", duration.as_nanos(), file_size).into())
}

fn try_revision_token(root: &Path, path: &Path) -> Option<RevisionToken> {
    let full = root.join(path);
    let meta = std::fs::metadata(&full).ok()?;
    revision_token(path, &meta).ok()
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

pub(crate) struct LocalStorage {
    path: PathBuf,
}

impl LocalStorage {
    pub fn new(path: PathBuf) -> Self {
        LocalStorage { path }
    }

    fn resolve(&self, path: &Path) -> PathBuf {
        self.path.join(path)
    }

    fn is_image(&self, ext: &str) -> bool {
        matches!(
            ext,
            "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "svg" | "ico" | "tiff" | "tif"
        )
    }

    async fn check_revision(
        &self,
        path: &Path,
        expected: &RevisionToken,
    ) -> Result<(), StorageError> {
        let full_path = self.resolve(path);
        let metadata = fs::metadata(&full_path)
            .await
            .map_err(|e| map_io_error(path, e))?;
        let actual = revision_token(path, &metadata)?;
        if *expected != actual {
            return Err(StorageError::Conflict(
                path.to_path_buf(),
                expected.clone(),
                actual,
            ));
        }
        Ok(())
    }
}

impl Storage for LocalStorage {
    async fn list(&self) -> Result<impl Stream<Item = PathBuf>, StorageError> {
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
                    } else if let Ok(relative) = path.strip_prefix(&root) {
                        yield relative.to_path_buf();
                    }
                }
            }
        })
    }

    async fn read(&self, path: PathBuf) -> Result<FileContent, StorageError> {
        let full_path = self.resolve(&path);
        let metadata = fs::metadata(&full_path)
            .await
            .map_err(|e| map_io_error(&path, e))?;

        let token = revision_token(&path, &metadata)?;
        let ext = full_path.extension().and_then(|e| e.to_str()).unwrap_or("");

        if self.is_image(ext) {
            let bytes = fs::read(&full_path)
                .await
                .map_err(|e| map_io_error(&path, e))?;
            let mime = mime_from_extension(ext).to_string();
            Ok(FileContent::Image {
                content: crate::core::common::DataURI::new(mime, &bytes),
                token,
            })
        } else {
            let content = fs::read_to_string(&full_path)
                .await
                .map_err(|e| map_io_error(&path, e))?;
            Ok(FileContent::Markdown { content, token })
        }
    }

    async fn write(&self, path: PathBuf, data: FileContent) -> Result<RevisionToken, StorageError> {
        let full_path = self.resolve(&path);

        if full_path.exists() {
            let expected = match &data {
                FileContent::Markdown { token, .. } => token,
                FileContent::Image { token, .. } => token,
            };
            self.check_revision(&path, expected).await?;
        }

        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|e| map_io_error(&path, e))?;
        }

        match data {
            FileContent::Markdown { content, .. } => {
                fs::write(&full_path, content.as_bytes())
                    .await
                    .map_err(|e| map_io_error(&path, e))?;
            }
            FileContent::Image {
                content: data_uri, ..
            } => {
                let bytes = data_uri
                    .decode()
                    .map_err(|e| StorageError::InvalidData(path.clone(), e.to_string()))?;
                fs::write(&full_path, bytes)
                    .await
                    .map_err(|e| map_io_error(&path, e))?;
            }
        }

        let metadata = fs::metadata(&full_path)
            .await
            .map_err(|e| map_io_error(&path, e))?;
        revision_token(&path, &metadata)
    }

    async fn delete(
        &self,
        path: PathBuf,
        expected_token: RevisionToken,
    ) -> Result<(), StorageError> {
        self.check_revision(&path, &expected_token).await?;

        let full_path = self.resolve(&path);
        fs::remove_file(&full_path)
            .await
            .map_err(|e| map_io_error(&path, e))
    }

    async fn rename(
        &self,
        from: PathBuf,
        to: PathBuf,
        expected_token: RevisionToken,
    ) -> Result<(), StorageError> {
        self.check_revision(&from, &expected_token).await?;

        let full_from = self.resolve(&from);
        let full_to = self.resolve(&to);

        if let Some(parent) = full_to.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|e| map_io_error(&to, e))?;
        }

        fs::rename(&full_from, &full_to)
            .await
            .map_err(|e| map_io_error(&from, e))
    }

    async fn copy(&self, from: PathBuf, to: PathBuf) -> Result<RevisionToken, StorageError> {
        let full_from = self.resolve(&from);
        let full_to = self.resolve(&to);

        if let Some(parent) = full_to.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|e| map_io_error(&to, e))?;
        }

        fs::copy(&full_from, &full_to)
            .await
            .map_err(|e| map_io_error(&from, e))?;

        let metadata = fs::metadata(&full_to)
            .await
            .map_err(|e| map_io_error(&to, e))?;
        revision_token(&to, &metadata)
    }

    async fn is_exists(&self, path: PathBuf) -> Result<bool, StorageError> {
        let full_path = self.resolve(&path);
        Ok(full_path.exists())
    }
}

impl StorageEventListener for LocalStorage {
    async fn listen(&self) -> Result<impl Stream<Item = StorageEvent>, StorageError> {
        let root = self.path.clone();
        let (tx, mut rx) = mpsc::channel(256);

        let mut watcher = RecommendedWatcher::new(
            move |res: Result<notify::Event, notify::Error>| {
                if let Ok(event) = res {
                    let _ = tx.blocking_send(event);
                }
            },
            notify::Config::default(),
        )
        .map_err(|e| StorageError::Io(self.path.clone(), std::io::Error::other(e)))?;

        watcher
            .watch(&self.path, RecursiveMode::Recursive)
            .map_err(|e| StorageError::Io(self.path.clone(), std::io::Error::other(e)))?;

        Ok(stream! {
            let _watcher = watcher;

            while let Some(event) = rx.recv().await {
                let paths: Vec<PathBuf> = event
                    .paths
                    .iter()
                    .filter_map(|p: &PathBuf| p.strip_prefix(&root).ok().map(|r| r.to_path_buf()))
                    .collect();

                if paths.is_empty() {
                    continue;
                }

                match event.kind {
                    EventKind::Create(_) => {
                        for path in paths {
                            if let Some(token) = try_revision_token(&root, &path) {
                                yield StorageEvent::Created { path, token };
                            }
                        }
                    }
                    EventKind::Modify(_) => {
                        for path in paths {
                            if let Some(token) = try_revision_token(&root, &path) {
                                yield StorageEvent::Updated { path, token };
                            }
                        }
                    }
                    EventKind::Remove(_) => {
                        for path in paths {
                            yield StorageEvent::Deleted { path };
                        }
                    }
                    _ => {}
                }
            }
        })
    }
}
