use futures_core::Stream;
use serde::{Serialize, de::DeserializeOwned};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use thiserror::Error;
use tokio_stream::StreamExt;

use crate::common::{Configurable, RevisionToken, VaultPath};
use crate::core::config::TarnConfig;
use crate::index::{Index, IndexError, IndexLink, NoteResult};
use crate::note_handler::{Frontmatter, FrontmatterValue, Note, NoteHandlerError, Section};
use crate::observer::{Observer, ObserverError, StorageEvent};
use crate::revisions::RevisionTracker;
use crate::storage::{File, FileContent, FileMeta, Storage, StorageError};

#[derive(Debug, Error)]
pub enum CoreError {
    #[error(transparent)]
    Storage(#[from] StorageError),
    #[error(transparent)]
    Index(#[from] IndexError),
    #[error(transparent)]
    Observer(#[from] ObserverError),
    #[error("note not found: {0}")]
    NoteNotFound(VaultPath),
    #[error("not a markdown file: {0}")]
    NotMarkdown(VaultPath),
    #[error("not a folder: {0}")]
    NotFolder(VaultPath),
    #[error("parse error: {0}")]
    Parse(#[from] NoteHandlerError),
    #[error("no match found for: {0}")]
    NoMatch(String),
    #[error("invalid regex: {0}")]
    InvalidRegex(#[from] regex::Error),
}

/// Mode for find-and-replace operations.
#[derive(Debug, Clone)]
pub enum UpdateMode {
    /// Literal string replacement (all occurrences).
    Text,
    /// Regex pattern replacement.
    Regex,
}

pub struct TarnCore<S, I, O, R> {
    storage: Arc<S>,
    vault_name: String,
    index: Arc<I>,
    observer: Arc<O>,
    revisions: Arc<R>,
}

impl<S, I, O, R> TarnCore<S, I, O, R> {
    pub fn new(
        storage: Arc<S>,
        vault_name: String,
        index: Arc<I>,
        observer: Arc<O>,
        revisions: Arc<R>,
    ) -> Self {
        Self {
            storage,
            vault_name,
            index,
            observer,
            revisions,
        }
    }
}

impl<S, I, O, R> Configurable for TarnCore<S, I, O, R>
where
    S: Storage + Configurable + Send + Sync + 'static,
    I: Index + Configurable + Send + Sync + 'static,
    O: Observer + Configurable + Send + Sync + 'static,
    R: RevisionTracker + Configurable + Send + Sync + 'static,
    S::Config: Serialize + DeserializeOwned,
    I::Config: Serialize + DeserializeOwned,
    O::Config: Serialize + DeserializeOwned,
    R::Config: Serialize + DeserializeOwned,
{
    type Config = TarnConfig<S::Config, <I as Configurable>::Config, O::Config, R::Config>;

    fn config(&self) -> Self::Config {
        TarnConfig {
            vault_name: self.vault_name.clone(),
            storage: self.storage.config(),
            index: self.index.config(),
            observer: self.observer.config(),
            revisions: self.revisions.config(),
        }
    }
}

impl<S, I, O, R> TarnCore<S, I, O, R>
where
    S: Storage + Send + Sync + 'static,
    I: Index + Send + Sync + 'static,
    O: Observer + Send + Sync + 'static,
    R: RevisionTracker + Send + Sync + 'static,
{
    /// Listen for live file changes from the observer.
    ///
    /// Returns a stream of `StorageEvent`s. The caller is responsible for
    /// consuming the stream and calling `update_index`/`delete_index` as needed.
    pub async fn listen_changes(
        &self,
    ) -> Result<impl Stream<Item = Result<StorageEvent, CoreError>>, CoreError> {
        let stream = self.observer.observe().await?;
        Ok(stream.map(Ok))
    }

    /// Review changes that occurred while the application was offline.
    ///
    /// Compares current storage state with the revision tracker and yields
    /// events for new, updated, or deleted files.
    pub async fn review_changes(
        &self,
    ) -> Result<impl Stream<Item = Result<StorageEvent, CoreError>> + '_, CoreError> {
        let storage_stream = self.storage.list(&VaultPath::Root).await?;
        tokio::pin!(storage_stream);
        let files: Vec<_> = storage_stream.collect().await;
        let storage_map: HashMap<VaultPath, RevisionToken> = files
            .into_iter()
            .map(|meta| (meta.path, meta.revision_token))
            .collect();

        let tracked_map: HashMap<VaultPath, RevisionToken> =
            self.revisions.all_revisions().await.into_iter().collect();

        let storage_keys: HashSet<&VaultPath> = storage_map.keys().collect();
        let tracked_keys: HashSet<&VaultPath> = tracked_map.keys().collect();

        let mut events: Vec<Result<StorageEvent, CoreError>> = Vec::new();

        // New files: in storage but not tracked
        for path in storage_keys.difference(&tracked_keys) {
            events.push(Ok(StorageEvent::Created {
                path: (*path).clone(),
                token: storage_map[*path].clone(),
            }));
        }

        // Deleted files: tracked but not in storage
        for path in tracked_keys.difference(&storage_keys) {
            events.push(Ok(StorageEvent::Deleted {
                path: (*path).clone(),
            }));
        }

        // Updated files: in both, but revision mismatch
        for path in storage_keys.intersection(&tracked_keys) {
            if storage_map[*path] != tracked_map[*path] {
                events.push(Ok(StorageEvent::Updated {
                    path: (*path).clone(),
                    token: storage_map[*path].clone(),
                }));
            }
        }

        Ok(tokio_stream::iter(events))
    }

    /// Update the index and revision tracker with a file's content.
    ///
    /// For markdown files, parses the content and updates the search index.
    /// For all files, updates the revision tracker.
    pub async fn update_index(&self, file: &File) -> Result<(), CoreError> {
        self.revisions
            .update_revision(&file.meta.path, file.meta.revision_token.clone())
            .await;

        if file.meta.path.is_note()
            && let FileContent::Markdown(content) = &file.content
        {
            let mut note = Note::from(content.as_str());
            note.path = Some(file.meta.path.clone());
            self.index.update(&note).await?;
        }

        Ok(())
    }

    /// Remove a path from the index and revision tracker.
    pub async fn delete_index(&self, path: &VaultPath) -> Result<(), CoreError> {
        self.revisions.remove_revision(path).await;
        if path.is_note() {
            self.index.remove(path).await?;
        }
        Ok(())
    }

    /// Look up the tracked revision for a path, or fail with `NoteNotFound`.
    async fn tracked_revision(&self, path: &VaultPath) -> Result<RevisionToken, CoreError> {
        self.revisions
            .get_revision(path)
            .await
            .ok_or_else(|| CoreError::NoteNotFound(path.clone()))
    }

    /// Read a file from the storage.
    pub async fn read(&self, path: &VaultPath) -> Result<File, CoreError> {
        Ok(self.storage.read(path).await?)
    }

    /// Parse markdown content into a structured Note.
    pub fn parse_content(&self, content: &str) -> Result<Note, CoreError> {
        Ok(Note::try_parse(content)?)
    }

    /// Create a new file in the storage.
    ///
    /// Fails with `StorageError::FileAlreadyExists` if the file already exists.
    pub async fn create(
        &self,
        path: &VaultPath,
        content: FileContent,
    ) -> Result<FileMeta, CoreError> {
        let meta = self.storage.write(path, content, None).await?;
        self.revisions
            .update_revision(path, meta.revision_token.clone())
            .await;
        Ok(meta)
    }

    /// Write file content to the storage.
    ///
    /// Creates the file if the revision tracker has no entry for it (no known
    /// prior revision), or overwrites it using the tracked revision token for
    /// optimistic concurrency control. Fails with a conflict error if the file
    /// has changed since it was last tracked.
    ///
    /// Requires `start_sync` to have completed before writing existing files,
    /// so that their revision tokens are already tracked.
    pub async fn write(
        &self,
        path: &VaultPath,
        content: FileContent,
    ) -> Result<FileMeta, CoreError> {
        let expected_token = self.revisions.get_revision(path).await;
        let meta = self.storage.write(path, content, expected_token).await?;
        self.revisions
            .update_revision(path, meta.revision_token.clone())
            .await;
        Ok(meta)
    }

    /// Find and replace content within a file.
    ///
    /// In `Text` mode the `old` string is replaced literally (all occurrences).
    /// In `Regex` mode `old` is compiled as a regex pattern.
    /// Returns an error if no match is found.
    pub async fn update(
        &self,
        path: &VaultPath,
        old: &str,
        new: &str,
        mode: UpdateMode,
    ) -> Result<FileMeta, CoreError> {
        let file = self.storage.read(path).await?;
        let current = match file.content {
            FileContent::Markdown(c) => c,
            _ => return Err(CoreError::NotMarkdown(path.clone())),
        };

        let replaced = match mode {
            UpdateMode::Text => {
                let result = current.replace(old, new);
                if result == current {
                    return Err(CoreError::NoMatch(old.to_string()));
                }
                result
            }
            UpdateMode::Regex => {
                let re = regex::Regex::new(old)?;
                let result = re.replace_all(&current, new).into_owned();
                if result == current {
                    return Err(CoreError::NoMatch(old.to_string()));
                }
                result
            }
        };

        self.write(path, FileContent::Markdown(replaced)).await
    }

    /// Delete a file. Fails if the file is not tracked.
    pub async fn delete(&self, path: &VaultPath) -> Result<(), CoreError> {
        let revision = self.tracked_revision(path).await?;

        self.storage.delete(path, revision).await?;
        self.revisions.remove_revision(path).await;
        Ok(())
    }

    /// Rename/move a file, optionally updating wikilinks in other notes.
    ///
    /// The file is moved first to ensure vault consistency: if the move fails,
    /// no backlinks have been modified. Backlink updates are best-effort after
    /// the move succeeds.
    ///
    /// When `update_links` is true, wikilinks referencing the old path are rewritten.
    /// Returns the new file metadata and the number of notes whose links were updated.
    pub async fn rename(
        &self,
        from: &VaultPath,
        to: &VaultPath,
        update_links: bool,
    ) -> Result<(FileMeta, usize), CoreError> {
        let revision = self.tracked_revision(from).await?;

        // Move the file first — if this fails, nothing else has changed
        let meta = self.storage.r#move(from, to, revision).await?;

        // Update tracker
        self.revisions.remove_revision(from).await;
        self.revisions
            .update_revision(to, meta.revision_token.clone())
            .await;

        // Update wikilinks in notes that reference the old path.
        // backlinks(from) still works because the index hasn't been updated yet.
        let old_stem = from.stem();
        let new_stem = to.stem();
        let mut links_updated = 0;

        if update_links && old_stem != new_stem {
            let backlink_paths = self.backlinks(from).await?;
            let pattern = format!(r"\[\[(?:[^\]#|]*/)?{}(\]\]|[#|])", regex::escape(old_stem));
            let replacement = format!("[[{new_stem}$1");
            for path in &backlink_paths {
                if self
                    .update(path, &pattern, &replacement, UpdateMode::Regex)
                    .await
                    .is_ok()
                {
                    links_updated += 1;
                }
            }
        }

        // Update index
        self.delete_index(from).await?;
        if to.is_note() {
            let file = self.storage.read(to).await?;
            self.update_index(&file).await?;
        }

        Ok((meta, links_updated))
    }

    /// List paths under a folder.
    ///
    /// Accepts `VaultPath::Root` or `VaultPath::Folder`. Always recursive.
    pub async fn list_paths(&self, path: &VaultPath) -> Result<Vec<VaultPath>, CoreError> {
        let folder = match path {
            VaultPath::Root => None,
            VaultPath::Folder(_) => Some(path),
            _ => return Err(CoreError::NotFolder(path.clone())),
        };
        let results = self.index.list(folder, true).await?;
        let mut paths: Vec<VaultPath> = results.into_iter().map(|r| r.path).collect();
        paths.sort();
        Ok(paths)
    }

    /// List sections within a note.
    ///
    /// Reads and parses the note, returning a `VaultPath::Section` for each section.
    pub async fn list_sections(&self, path: &VaultPath) -> Result<Vec<VaultPath>, CoreError> {
        let file = self.storage.read(path).await?;
        let content = match file.content {
            FileContent::Markdown(c) => c,
            _ => return Err(CoreError::NotMarkdown(path.clone())),
        };
        let note = self.parse_content(&content)?;
        let note_str = path.as_str();

        let sections = note
            .sections
            .iter()
            .filter(|s| !s.heading_path.is_empty())
            .filter_map(|s| {
                let section_path = format!("{}#{}", note_str, s.heading_path.join("/"));
                VaultPath::new(&section_path).ok()
            })
            .collect();

        Ok(sections)
    }

    /// Check if a file or section exists.
    ///
    /// For notes/images/folders: checks the revision tracker.
    /// For sections: checks the index.
    pub async fn exists(&self, path: &VaultPath) -> Result<bool, CoreError> {
        match path {
            VaultPath::Section(_) => {
                let note_path = path
                    .note_path()
                    .ok_or_else(|| CoreError::NotMarkdown(path.clone()))?;
                let entries = self.index.get(&note_path).await?;
                let headings = path.section_headings();
                Ok(entries
                    .iter()
                    .any(|e| e.path.section_headings() == headings))
            }
            _ => Ok(self.revisions.get_revision(path).await.is_some()),
        }
    }

    /// Search for notes matching a query. Returns note-level results with scores.
    pub async fn search(
        &self,
        query: &str,
        folders: &[VaultPath],
        tags: &[String],
        limit: usize,
        token_limit: Option<usize>,
        score_threshold: f32,
    ) -> Result<Vec<NoteResult>, CoreError> {
        let results = self
            .index
            .search(query, folders, tags, limit, token_limit, score_threshold)
            .await?;
        Ok(results)
    }

    /// Get tags with counts, optionally filtered by prefix and folder.
    ///
    /// Returns a flat map of tag names to occurrence counts.
    /// Hierarchy building is the caller's responsibility.
    pub async fn list_tags(
        &self,
        prefix: Option<&str>,
        folder: Option<&VaultPath>,
    ) -> Result<HashMap<String, usize>, CoreError> {
        let results = self.index.list(folder, true).await?;
        let mut tag_counts: HashMap<String, usize> = HashMap::new();

        for result in &results {
            for tag in result.tags() {
                *tag_counts.entry(tag).or_default() += 1;
            }
        }

        if let Some(p) = prefix {
            tag_counts.retain(|tag, _| tag.starts_with(p));
        }

        Ok(tag_counts)
    }

    /// Get note paths that link to the given target.
    pub async fn backlinks(&self, target: &VaultPath) -> Result<HashSet<VaultPath>, CoreError> {
        let results = self.index.backlinks(target.stem()).await?;
        Ok(results.into_iter().map(|r| r.path).collect())
    }

    /// Get all note paths linked from the given note.
    ///
    /// Resolves wiki link targets to `VaultPath::Note`. Skips URLs and emails.
    pub async fn forward_links(&self, path: &VaultPath) -> Result<HashSet<VaultPath>, CoreError> {
        let links = self.index.forward_links(path).await?;
        let mut paths = HashSet::new();
        for link in links {
            match link {
                IndexLink::Wiki { target, .. } => {
                    let note_path = if target.ends_with(".md") {
                        target
                    } else {
                        format!("{target}.md")
                    };
                    if let Ok(vp) = VaultPath::new(&note_path) {
                        paths.insert(vp);
                    }
                }
                IndexLink::Markdown { url, .. } if !url.starts_with("http") => {
                    if let Ok(vp) = VaultPath::new(&url) {
                        paths.insert(vp);
                    }
                }
                _ => {} // Skip URLs and emails
            }
        }
        Ok(paths)
    }

    /// Get the vault name.
    pub fn vault_name(&self) -> &str {
        &self.vault_name
    }

    /// Read the frontmatter from a note file.
    pub async fn read_frontmatter(&self, path: &VaultPath) -> Result<Frontmatter, CoreError> {
        let file = self.storage.read(path).await?;
        let content = match file.content {
            FileContent::Markdown(c) => c,
            _ => return Err(CoreError::NotMarkdown(path.clone())),
        };
        let note = self.parse_content(&content)?;
        Ok(note.frontmatter.unwrap_or_default())
    }

    /// Replace the entire frontmatter of a note.
    pub async fn write_frontmatter(
        &self,
        path: &VaultPath,
        frontmatter: Frontmatter,
    ) -> Result<FileMeta, CoreError> {
        let file = self.storage.read(path).await?;
        let content = match file.content {
            FileContent::Markdown(c) => c,
            _ => return Err(CoreError::NotMarkdown(path.clone())),
        };
        let mut note = self.parse_content(&content)?;
        note.frontmatter = Some(frontmatter);
        self.write(path, FileContent::Markdown(note.to_string()))
            .await
    }

    /// Update specific frontmatter values: remove keys first, then set values.
    pub async fn update_frontmatter(
        &self,
        path: &VaultPath,
        set: HashMap<String, FrontmatterValue>,
        remove: Vec<String>,
    ) -> Result<FileMeta, CoreError> {
        let file = self.storage.read(path).await?;
        let content = match file.content {
            FileContent::Markdown(c) => c,
            _ => return Err(CoreError::NotMarkdown(path.clone())),
        };
        let mut note = self.parse_content(&content)?;
        let frontmatter = note.frontmatter.get_or_insert_with(Frontmatter::default);

        // Serialize to JSON map for uniform key handling
        let mut map = serde_json::to_value(&*frontmatter)
            .map_err(|e| CoreError::Parse(NoteHandlerError::InvalidFrontmatter(e.to_string())))?;
        let obj = map.as_object_mut().ok_or_else(|| {
            CoreError::Parse(NoteHandlerError::InvalidFrontmatter(
                "not an object".to_string(),
            ))
        })?;

        // Remove first
        for key in &remove {
            obj.remove(key);
        }

        // Then set
        for (key, value) in set {
            let json_value = serde_json::to_value(&value).map_err(|e| {
                CoreError::Parse(NoteHandlerError::InvalidFrontmatter(e.to_string()))
            })?;
            obj.insert(key, json_value);
        }

        // Deserialize back
        let updated: Frontmatter = serde_json::from_value(map)
            .map_err(|e| CoreError::Parse(NoteHandlerError::InvalidFrontmatter(e.to_string())))?;
        note.frontmatter = Some(updated);
        self.write(path, FileContent::Markdown(note.to_string()))
            .await
    }

    /// Remove the entire frontmatter block from a note.
    pub async fn delete_frontmatter(&self, path: &VaultPath) -> Result<FileMeta, CoreError> {
        let file = self.storage.read(path).await?;
        let content = match file.content {
            FileContent::Markdown(c) => c,
            _ => return Err(CoreError::NotMarkdown(path.clone())),
        };
        let mut note = self.parse_content(&content)?;
        note.frontmatter = None;
        self.write(path, FileContent::Markdown(note.to_string()))
            .await
    }

    /// Append content to the end of a note.
    ///
    /// Reads the file fresh from storage and appends the new content.
    /// Uses storage-level revision check — may fail with a conflict error
    /// if the file is modified between the read and write.
    pub async fn append(
        &self,
        path: &VaultPath,
        content: FileContent,
    ) -> Result<FileMeta, CoreError> {
        let file = self.storage.read(path).await?;
        let current = match file.content {
            FileContent::Markdown(c) => c,
            _ => return Err(CoreError::NotMarkdown(path.clone())),
        };
        let appended = match content {
            FileContent::Markdown(new) => format!("{current}\n{new}"),
            _ => return Err(CoreError::NotMarkdown(path.clone())),
        };
        let meta = self
            .storage
            .write(
                path,
                FileContent::Markdown(appended),
                Some(file.meta.revision_token),
            )
            .await?;
        self.revisions
            .update_revision(path, meta.revision_token.clone())
            .await;
        Ok(meta)
    }

    /// Copy a file to a new path.
    pub async fn copy(&self, from: &VaultPath, to: &VaultPath) -> Result<FileMeta, CoreError> {
        let meta = self.storage.copy(from, to).await?;
        self.revisions
            .update_revision(to, meta.revision_token.clone())
            .await;
        Ok(meta)
    }
}

// Static methods that don't depend on generic parameters
impl<S, I, O, R> TarnCore<S, I, O, R> {
    /// Find a section within a note by heading path.
    ///
    /// The heading path is matched hierarchically. For example,
    /// `["Goals", "Q1"]` matches a `## Q1` section under `# Goals`.
    pub fn resolve_section<'a>(note: &'a Note, heading_path: &[&str]) -> Option<&'a Section> {
        note.sections.iter().find(|s| {
            s.heading_path.len() == heading_path.len()
                && s.heading_path
                    .iter()
                    .zip(heading_path.iter())
                    .all(|(a, b)| a == b)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::Buildable;
    use crate::core::config::TarnConfig;
    use tempfile::TempDir;

    type TestCore = TarnCore<
        crate::storage::local::LocalStorage,
        crate::index::InMemoryIndex,
        crate::observer::LocalStorageObserver,
        crate::revisions::InMemoryRevisionTracker,
    >;

    fn setup() -> (TempDir, TestCore) {
        let dir = TempDir::new().unwrap();
        let core = TarnConfig::local(dir.path().to_path_buf()).build().unwrap();
        (dir, core)
    }

    fn vp(path: &str) -> VaultPath {
        VaultPath::new(path).unwrap()
    }

    fn md(content: &str) -> FileContent {
        FileContent::Markdown(content.to_string())
    }

    /// Write a file and immediately index it.
    async fn write_and_index(core: &TestCore, path: &VaultPath, content: FileContent) {
        core.write(path, content).await.unwrap();
        let file = core.read(path).await.unwrap();
        core.update_index(&file).await.unwrap();
    }

    #[tokio::test]
    async fn test_create_and_read() {
        let (_dir, core) = setup();
        core.create(&vp("hello.md"), md("# Hello\nWorld"))
            .await
            .unwrap();

        let file = core.read(&vp("hello.md")).await.unwrap();
        match file.content {
            FileContent::Markdown(content) => assert_eq!(content.trim(), "# Hello\nWorld"),
            _ => panic!("expected markdown"),
        }
    }

    #[tokio::test]
    async fn test_create_rejects_existing() {
        let (_dir, core) = setup();
        core.create(&vp("note.md"), md("v1")).await.unwrap();
        let err = core.create(&vp("note.md"), md("v2")).await.unwrap_err();
        assert!(
            matches!(err, CoreError::Storage(_)),
            "expected storage error, got: {err}"
        );
    }

    #[tokio::test]
    async fn test_write_and_read() {
        let (_dir, core) = setup();
        core.write(&vp("hello.md"), md("# Hello\nWorld"))
            .await
            .unwrap();

        let file = core.read(&vp("hello.md")).await.unwrap();
        match file.content {
            FileContent::Markdown(content) => assert_eq!(content.trim(), "# Hello\nWorld"),
            _ => panic!("expected markdown"),
        }
    }

    #[tokio::test]
    async fn test_write_returns_file_meta() {
        let (_dir, core) = setup();
        let meta = core.write(&vp("note.md"), md("content")).await.unwrap();
        assert_eq!(meta.path, vp("note.md"));
    }

    #[tokio::test]
    async fn test_write_overwrites_existing() {
        let (_dir, core) = setup();
        core.write(&vp("note.md"), md("v1")).await.unwrap();
        core.write(&vp("note.md"), md("v2")).await.unwrap();

        let file = core.read(&vp("note.md")).await.unwrap();
        match file.content {
            FileContent::Markdown(content) => assert_eq!(content.trim(), "v2"),
            _ => panic!("expected markdown"),
        }
    }

    #[tokio::test]
    async fn test_exists_reflects_tracker() {
        let (_dir, core) = setup();
        assert!(!core.exists(&vp("missing.md")).await.unwrap());

        core.write(&vp("note.md"), md("content")).await.unwrap();
        assert!(core.exists(&vp("note.md")).await.unwrap());
    }

    #[tokio::test]
    async fn test_update_text_replace() {
        let (_dir, core) = setup();
        core.write(&vp("note.md"), md("hello world hello"))
            .await
            .unwrap();
        core.update(&vp("note.md"), "hello", "hi", UpdateMode::Text)
            .await
            .unwrap();

        let file = core.read(&vp("note.md")).await.unwrap();
        match file.content {
            FileContent::Markdown(content) => assert_eq!(content.trim(), "hi world hi"),
            _ => panic!("expected markdown"),
        }
    }

    #[tokio::test]
    async fn test_update_regex_replace() {
        let (_dir, core) = setup();
        core.write(&vp("note.md"), md("foo123bar")).await.unwrap();
        core.update(&vp("note.md"), r"\d+", "NUM", UpdateMode::Regex)
            .await
            .unwrap();

        let file = core.read(&vp("note.md")).await.unwrap();
        match file.content {
            FileContent::Markdown(content) => assert_eq!(content.trim(), "fooNUMbar"),
            _ => panic!("expected markdown"),
        }
    }

    #[tokio::test]
    async fn test_update_no_match_fails() {
        let (_dir, core) = setup();
        core.write(&vp("note.md"), md("content")).await.unwrap();
        let err = core
            .update(&vp("note.md"), "missing", "new", UpdateMode::Text)
            .await
            .unwrap_err();
        assert!(matches!(err, CoreError::NoMatch(_)));
    }

    #[tokio::test]
    async fn test_delete() {
        let (_dir, core) = setup();
        core.write(&vp("note.md"), md("content")).await.unwrap();
        core.delete(&vp("note.md")).await.unwrap();
        assert!(!core.exists(&vp("note.md")).await.unwrap());
    }

    #[tokio::test]
    async fn test_delete_untracked_fails() {
        let (_dir, core) = setup();
        let err = core.delete(&vp("note.md")).await.unwrap_err();
        assert!(matches!(err, CoreError::NoteNotFound(_)));
    }

    #[tokio::test]
    async fn test_parse_content_and_resolve_section() {
        let (_dir, core) = setup();
        let note = core
            .parse_content("# Top\n\ncontent\n\n## Sub\n\nsub content")
            .unwrap();
        assert_eq!(note.title, Some("Top".to_string()));
        assert_eq!(note.sections.len(), 2);

        let section = TestCore::resolve_section(&note, &["Top", "Sub"]);
        assert!(section.is_some());
        assert!(section.unwrap().content.contains("sub content"));

        let missing = TestCore::resolve_section(&note, &["Nonexistent"]);
        assert!(missing.is_none());
    }

    #[tokio::test]
    async fn test_search_empty_query_returns_empty() {
        let (_dir, core) = setup();
        write_and_index(&core, &vp("note.md"), md("content")).await;

        let hits = core.search("", &[], &[], 10, None, 0.0).await.unwrap();
        assert!(hits.is_empty());
    }

    #[tokio::test]
    async fn test_search_finds_content() {
        let (_dir, core) = setup();
        write_and_index(
            &core,
            &vp("note.md"),
            md("# Rust\n\nRust is a systems language"),
        )
        .await;

        let hits = core
            .search("systems", &[], &[], 10, None, 0.0)
            .await
            .unwrap();
        assert!(!hits.is_empty());
        assert!(hits[0].path.to_string().starts_with("note.md"));
    }

    #[tokio::test]
    async fn test_list_tags() {
        let (_dir, core) = setup();
        write_and_index(
            &core,
            &vp("note.md"),
            md("---\ntags:\n  - rust\n  - programming\n---\n# Note"),
        )
        .await;

        let tags = core.list_tags(None, None).await.unwrap();
        assert!(tags.contains_key("rust"));
        assert!(tags.contains_key("programming"));
        assert_eq!(*tags.get("rust").unwrap(), 1);
    }

    #[tokio::test]
    async fn test_list_paths() {
        let (_dir, core) = setup();
        write_and_index(&core, &vp("a.md"), md("# A")).await;
        write_and_index(&core, &vp("b.md"), md("# B")).await;

        let paths = core.list_paths(&VaultPath::Root).await.unwrap();
        assert_eq!(paths.len(), 2);
    }

    #[tokio::test]
    async fn test_list_sections() {
        let (_dir, core) = setup();
        core.write(
            &vp("note.md"),
            md("# Top\n\ncontent\n\n## Sub\n\nsub content"),
        )
        .await
        .unwrap();

        let sections = core.list_sections(&vp("note.md")).await.unwrap();
        assert_eq!(sections.len(), 2);
        assert!(sections.iter().any(|s| s.is_section()));
    }

    #[tokio::test]
    async fn test_vault_name() {
        let (_dir, core) = setup();
        assert!(!core.vault_name().is_empty());
    }

    #[tokio::test]
    async fn test_rename() {
        let (_dir, core) = setup();
        core.write(&vp("old.md"), md("# Old")).await.unwrap();

        let (meta, links_updated) = core
            .rename(&vp("old.md"), &vp("new.md"), true)
            .await
            .unwrap();
        assert_eq!(meta.path, vp("new.md"));
        assert_eq!(links_updated, 0);
        assert!(!core.exists(&vp("old.md")).await.unwrap());
        assert!(core.exists(&vp("new.md")).await.unwrap());
    }

    #[tokio::test]
    async fn test_rename_untracked_fails() {
        let (_dir, core) = setup();
        let err = core
            .rename(&vp("old.md"), &vp("new.md"), true)
            .await
            .unwrap_err();
        assert!(matches!(err, CoreError::NoteNotFound(_)));
    }

    #[tokio::test]
    async fn test_backlinks() {
        let (_dir, core) = setup();
        write_and_index(&core, &vp("source.md"), md("See [[target]] for details.")).await;
        write_and_index(&core, &vp("target.md"), md("# Target")).await;

        let backlinks = core.backlinks(&vp("target.md")).await.unwrap();
        assert_eq!(backlinks.len(), 1);
        assert!(backlinks.contains(&vp("source.md")));
    }

    #[tokio::test]
    async fn test_forward_links() {
        let (_dir, core) = setup();
        write_and_index(
            &core,
            &vp("note.md"),
            md("Links to [[alpha]] and [[beta|B]]."),
        )
        .await;

        let links = core.forward_links(&vp("note.md")).await.unwrap();
        assert_eq!(links.len(), 2);
        assert!(links.contains(&vp("alpha.md")));
        assert!(links.contains(&vp("beta.md")));
    }

    #[tokio::test]
    async fn test_read_nonexistent_fails() {
        let (_dir, core) = setup();
        let err = core.read(&vp("missing.md")).await.unwrap_err();
        assert!(matches!(err, CoreError::Storage(StorageError::NotFound(_))));
    }

    // --- Revision tracking tests ---

    #[tokio::test]
    async fn test_write_updates_revision_tracker() {
        let (_dir, core) = setup();
        let path = vp("note.md");
        core.write(&path, md("content")).await.unwrap();
        assert!(core.revisions.get_revision(&path).await.is_some());
    }

    #[tokio::test]
    async fn test_write_overwrite_updates_revision_tracker() {
        let (_dir, core) = setup();
        let path = vp("note.md");
        core.write(&path, md("v1")).await.unwrap();
        let rev1 = core.revisions.get_revision(&path).await.unwrap();

        core.write(&path, md("v2")).await.unwrap();
        let rev2 = core.revisions.get_revision(&path).await.unwrap();
        assert_ne!(rev1, rev2);
    }

    #[tokio::test]
    async fn test_delete_removes_from_revision_tracker() {
        let (_dir, core) = setup();
        let path = vp("note.md");
        core.write(&path, md("content")).await.unwrap();
        core.delete(&path).await.unwrap();
        assert!(core.revisions.get_revision(&path).await.is_none());
    }

    #[tokio::test]
    async fn test_rename_updates_revision_tracker() {
        let (_dir, core) = setup();
        let old_path = vp("old.md");
        let new_path = vp("new.md");
        core.write(&old_path, md("# Old")).await.unwrap();
        let _ = core.rename(&old_path, &new_path, true).await.unwrap();

        assert!(core.revisions.get_revision(&old_path).await.is_none());
        assert!(core.revisions.get_revision(&new_path).await.is_some());
    }

    #[tokio::test]
    async fn test_review_changes_detects_updated() {
        let (_dir, core) = setup();
        let path = vp("note.md");
        core.write(&path, md("v1")).await.unwrap();
        let rev = core.revisions.get_revision(&path).await.unwrap();

        // Modify file directly via storage to create mismatch
        let _new_meta = core
            .storage
            .write(
                &path,
                FileContent::Markdown("v2".to_string()),
                Some(rev.clone()),
            )
            .await
            .unwrap();

        // review_changes should detect the mismatch
        let stream = core.review_changes().await.unwrap();
        tokio::pin!(stream);
        let events: Vec<_> = stream.collect().await;
        assert!(events.iter().any(|e| matches!(
            e,
            Ok(StorageEvent::Updated { path: p, .. }) if *p == path
        )));
    }

    #[tokio::test]
    async fn test_review_changes_detects_deleted() {
        let (_dir, core) = setup();
        let path = vp("note.md");
        core.write(&path, md("content")).await.unwrap();
        let rev = core.revisions.get_revision(&path).await.unwrap();

        // Delete directly via storage
        core.storage.delete(&path, rev).await.unwrap();

        // review_changes should detect the deletion
        let stream = core.review_changes().await.unwrap();
        tokio::pin!(stream);
        let events: Vec<_> = stream.collect().await;
        assert!(events.iter().any(|e| matches!(
            e,
            Ok(StorageEvent::Deleted { path: p }) if *p == path
        )));
    }

    #[tokio::test]
    async fn test_update_index_and_delete_index() {
        let (_dir, core) = setup();
        let path = vp("note.md");
        core.write(&path, md("# Test\n\nsearchable content"))
            .await
            .unwrap();
        let file = core.read(&path).await.unwrap();
        core.update_index(&file).await.unwrap();

        // Should be searchable now
        let hits = core
            .search("searchable", &[], &[], 10, None, 0.0)
            .await
            .unwrap();
        assert!(!hits.is_empty());

        // Delete from index
        core.delete_index(&path).await.unwrap();
        let hits = core
            .search("searchable", &[], &[], 10, None, 0.0)
            .await
            .unwrap();
        assert!(hits.is_empty());
    }

    // --- Frontmatter operations ---

    #[tokio::test]
    async fn test_write_frontmatter() {
        let (_dir, core) = setup();
        let path = vp("note.md");
        core.write(&path, md("# Hello\n\nBody")).await.unwrap();

        let fm = Frontmatter {
            title: Some("Hello".to_string()),
            tags: vec!["rust".to_string()],
            ..Default::default()
        };
        core.write_frontmatter(&path, fm).await.unwrap();

        let file = core.read(&path).await.unwrap();
        match file.content {
            FileContent::Markdown(c) => {
                assert!(c.contains("title: Hello"));
                assert!(c.contains("rust"));
                assert!(c.contains("# Hello"));
            }
            _ => panic!("expected markdown"),
        }
    }

    #[tokio::test]
    async fn test_write_frontmatter_replaces_existing() {
        let (_dir, core) = setup();
        let path = vp("note.md");
        core.write(&path, md("---\ntitle: Old\ntags:\n  - old\n---\n# Note"))
            .await
            .unwrap();

        let fm = Frontmatter {
            title: Some("New".to_string()),
            ..Default::default()
        };
        core.write_frontmatter(&path, fm).await.unwrap();

        let read_fm = core.read_frontmatter(&path).await.unwrap();
        assert_eq!(read_fm.title, Some("New".to_string()));
        assert!(read_fm.tags.is_empty()); // old tags gone
    }

    #[tokio::test]
    async fn test_update_frontmatter_set() {
        let (_dir, core) = setup();
        let path = vp("note.md");
        core.write(&path, md("---\ntitle: Original\n---\n# Note"))
            .await
            .unwrap();

        let mut set = HashMap::new();
        set.insert(
            "description".to_string(),
            FrontmatterValue::Str("A description".to_string()),
        );
        core.update_frontmatter(&path, set, vec![]).await.unwrap();

        let fm = core.read_frontmatter(&path).await.unwrap();
        assert_eq!(fm.title, Some("Original".to_string())); // preserved
        assert_eq!(fm.description, Some("A description".to_string()));
    }

    #[tokio::test]
    async fn test_update_frontmatter_remove() {
        let (_dir, core) = setup();
        let path = vp("note.md");
        core.write(
            &path,
            md("---\ntitle: Keep\ndescription: Remove me\n---\n# Note"),
        )
        .await
        .unwrap();

        core.update_frontmatter(&path, HashMap::new(), vec!["description".to_string()])
            .await
            .unwrap();

        let fm = core.read_frontmatter(&path).await.unwrap();
        assert_eq!(fm.title, Some("Keep".to_string()));
        assert_eq!(fm.description, None);
    }

    #[tokio::test]
    async fn test_update_frontmatter_remove_then_set() {
        let (_dir, core) = setup();
        let path = vp("note.md");
        core.write(&path, md("---\nstatus: draft\n---\n# Note"))
            .await
            .unwrap();

        // Remove "status", then set "status" to new value — set wins
        let mut set = HashMap::new();
        set.insert(
            "status".to_string(),
            FrontmatterValue::Str("published".to_string()),
        );
        core.update_frontmatter(&path, set, vec!["status".to_string()])
            .await
            .unwrap();

        let fm = core.read_frontmatter(&path).await.unwrap();
        assert_eq!(
            fm.custom.get("status"),
            Some(&FrontmatterValue::Str("published".to_string()))
        );
    }

    #[tokio::test]
    async fn test_update_frontmatter_creates_when_missing() {
        let (_dir, core) = setup();
        let path = vp("note.md");
        core.write(&path, md("# No frontmatter")).await.unwrap();

        let mut set = HashMap::new();
        set.insert(
            "title".to_string(),
            FrontmatterValue::Str("New Title".to_string()),
        );
        core.update_frontmatter(&path, set, vec![]).await.unwrap();

        let fm = core.read_frontmatter(&path).await.unwrap();
        assert_eq!(fm.title, Some("New Title".to_string()));
    }

    #[tokio::test]
    async fn test_delete_frontmatter() {
        let (_dir, core) = setup();
        let path = vp("note.md");
        core.write(&path, md("---\ntitle: Remove\n---\n# Note\n\nBody"))
            .await
            .unwrap();

        core.delete_frontmatter(&path).await.unwrap();

        let file = core.read(&path).await.unwrap();
        match file.content {
            FileContent::Markdown(c) => {
                assert!(!c.contains("---"));
                assert!(c.contains("# Note"));
                assert!(c.contains("Body"));
            }
            _ => panic!("expected markdown"),
        }
    }

    // --- Append ---

    #[tokio::test]
    async fn test_append() {
        let (_dir, core) = setup();
        let path = vp("note.md");
        core.write(&path, md("# Note\n\nOriginal")).await.unwrap();

        core.append(&path, md("\nAppended content")).await.unwrap();

        let file = core.read(&path).await.unwrap();
        match file.content {
            FileContent::Markdown(c) => {
                assert!(c.contains("Original"));
                assert!(c.contains("Appended content"));
            }
            _ => panic!("expected markdown"),
        }
    }

    #[tokio::test]
    async fn test_append_nonexistent_fails() {
        let (_dir, core) = setup();
        let err = core
            .append(&vp("missing.md"), md("content"))
            .await
            .unwrap_err();
        assert!(matches!(err, CoreError::Storage(StorageError::NotFound(_))));
    }

    // --- Copy ---

    #[tokio::test]
    async fn test_copy() {
        let (_dir, core) = setup();
        let from = vp("source.md");
        let to = vp("dest.md");
        core.write(&from, md("# Source")).await.unwrap();

        let meta = core.copy(&from, &to).await.unwrap();
        assert_eq!(meta.path, to);
        assert!(core.exists(&from).await.unwrap());
        assert!(core.exists(&to).await.unwrap());
    }

    // --- Rename with link updates ---

    #[tokio::test]
    async fn test_rename_updates_wikilinks() {
        let (_dir, core) = setup();
        write_and_index(&core, &vp("linker.md"), md("See [[target]] for details.")).await;
        write_and_index(&core, &vp("target.md"), md("# Target")).await;

        let (meta, links_updated) = core
            .rename(&vp("target.md"), &vp("renamed.md"), true)
            .await
            .unwrap();
        assert_eq!(meta.path, vp("renamed.md"));
        assert_eq!(links_updated, 1);

        // The linking note should now reference the new name
        let file = core.read(&vp("linker.md")).await.unwrap();
        match file.content {
            FileContent::Markdown(c) => {
                assert!(c.contains("[[renamed]]"), "expected updated link, got: {c}");
                assert!(!c.contains("[[target]]"));
            }
            _ => panic!("expected markdown"),
        }
    }

    #[tokio::test]
    async fn test_rename_same_stem_skips_link_update() {
        let (_dir, core) = setup();
        write_and_index(&core, &vp("linker.md"), md("See [[note]] here.")).await;
        write_and_index(&core, &vp("note.md"), md("# Note")).await;

        // Moving to a different folder but same stem — no link changes needed
        let (meta, links_updated) = core
            .rename(&vp("note.md"), &vp("sub/note.md"), true)
            .await
            .unwrap();
        assert_eq!(meta.path, vp("sub/note.md"));
        assert_eq!(links_updated, 0);
    }
}
