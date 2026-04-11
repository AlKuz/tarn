use serde::{Serialize, de::DeserializeOwned};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use thiserror::Error;
use tokio::task::JoinHandle;
use tokio_stream::StreamExt;
use tracing::{debug, info, warn};

use crate::common::{Configurable, RevisionToken, VaultPath};
use crate::core::config::TarnConfig;
use crate::core::responses::TagEntry;
use crate::index::find_direct_children;
use crate::index::{Index, IndexError, IndexLink, NoteResult};
use crate::note_handler::{Note, Section};
use crate::observer::{Observer, ObserverError, StorageEvent};
use crate::revisions::RevisionTracker;
use crate::storage::{FileContent, Storage, StorageError};

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
    #[error("invalid regex: {0}")]
    InvalidRegex(#[from] regex::Error),
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
    async fn read_and_parse(&self, path: &VaultPath) -> Result<Note, CoreError> {
        let file = self.storage.read(path).await?;
        match file.content {
            FileContent::Markdown(content) => {
                let mut note = Note::from(content.as_str());
                note.path = Some(path.clone());
                Ok(note)
            }
            FileContent::Image(_) => Err(CoreError::NotMarkdown(path.clone())),
        }
    }

    /// Rebuild the index from all notes in the vault.
    ///
    /// This clears the existing index and re-indexes all Markdown files.
    /// No-op if index is not configured.
    pub async fn rebuild_index(&self) -> Result<(), CoreError> {
        let index = &self.index;
        index.clear().await?;

        let stream = self.storage.list(&VaultPath::Root).await?;
        tokio::pin!(stream);
        let files = stream
            .filter(|meta| meta.path.is_note())
            .map(|meta| meta.path)
            .collect::<Vec<_>>()
            .await;
        let mut notes = Vec::new();

        for file_path in &files {
            match self.read_and_parse(file_path).await {
                Ok(note) => notes.push(note),
                Err(e) => {
                    warn!(path = %file_path, error = %e, "skipping note during index rebuild");
                }
            }
        }

        index.update_bulk(&notes).await?;
        Ok(())
    }

    /// Validate all tracked revisions against current storage state.
    ///
    /// Called at startup to reconcile the tracker with any changes that occurred
    /// while the server was offline. Updates mismatched tokens, removes entries
    /// for deleted files, and adds entries for new untracked files. Index
    /// entries are kept in sync with the tracker for any markdown file changes.
    pub async fn validate_revisions(&self) -> Result<(), CoreError> {
        let stream = self.storage.list(&VaultPath::Root).await?;
        tokio::pin!(stream);
        let files: Vec<_> = stream.collect().await;

        let tracked = self.revisions.all_revisions().await;
        let mut storage_paths: HashSet<VaultPath> = HashSet::new();

        for meta in &files {
            storage_paths.insert(meta.path.clone());
            let needs_sync = match self.revisions.get_revision(&meta.path).await {
                Some(stored) if stored != meta.revision_token => {
                    warn!(path = %meta.path, "revision mismatch, updating tracker and index");
                    true
                }
                None => true,
                _ => false,
            };

            if needs_sync {
                self.revisions
                    .update_revision(&meta.path, meta.revision_token.clone())
                    .await;
                self.reindex_path(&meta.path).await;
            }
        }

        for (path, _) in tracked {
            if !storage_paths.contains(&path) {
                warn!(path = %path, "tracked path no longer exists, removing from tracker and index");
                self.revisions.remove_revision(&path).await;
                if path.is_note()
                    && let Err(e) = self.index.remove(&path).await
                {
                    debug!(path = %path, error = %e, "failed to remove from index");
                }
            }
        }

        Ok(())
    }

    /// Read and re-index a single note path. Skips non-note paths and logs
    /// (but does not propagate) read/parse failures.
    async fn reindex_path(&self, path: &VaultPath) {
        if !path.is_note() {
            return;
        }
        match self.read_and_parse(path).await {
            Ok(note) => {
                if let Err(e) = self.index.update(&note).await {
                    debug!(path = %path, error = %e, "failed to update index");
                }
            }
            Err(e) => {
                debug!(path = %path, error = %e, "failed to read note for indexing");
            }
        }
    }

    /// Start background index synchronization.
    ///
    /// Spawns a task that watches for file changes and updates the index
    /// and revision tracker. Returns a handle to the background task.
    pub fn start_index_sync(&self) -> JoinHandle<()> {
        let index = self.index.clone();
        let storage = self.storage.clone();
        let observer = self.observer.clone();
        let revisions = self.revisions.clone();

        tokio::spawn(async move {
            let stream = match observer.observe().await {
                Ok(s) => s,
                Err(e) => {
                    warn!(error = %e, "failed to start file watcher");
                    return;
                }
            };
            tokio::pin!(stream);

            while let Some(event) = stream.next().await {
                match event {
                    StorageEvent::Created { path, token }
                    | StorageEvent::Updated { path, token } => {
                        // Update revision tracker with observed token
                        revisions.update_revision(&path, token).await;

                        if !path.is_note() {
                            continue;
                        }

                        match storage.read(&path).await {
                            Ok(file) => match file.content {
                                FileContent::Markdown(content) => {
                                    let mut note = Note::from(content.as_str());
                                    note.path = Some(path.clone());

                                    if let Err(e) = index.update(&note).await {
                                        debug!(path = %path, error = %e, "failed to update index (likely shutting down)");
                                    } else {
                                        info!(path = %path, "indexed note");
                                    }
                                }
                                FileContent::Image(_) => {
                                    // Skip images
                                }
                            },
                            Err(e) => {
                                debug!(path = %path, error = %e, "failed to read note for indexing (likely shutting down)");
                            }
                        }
                    }
                    StorageEvent::Deleted { path } => {
                        // Remove from revision tracker
                        revisions.remove_revision(&path).await;

                        if !path.is_note() {
                            continue;
                        }

                        if let Err(e) = index.remove(&path).await {
                            debug!(path = %path, error = %e, "failed to remove from index (likely shutting down)");
                        } else {
                            info!(path = %path, "removed note from index");
                        }
                    }
                }
            }
        })
    }

    /// Look up the tracked revision for a path, or fail with `NoteNotFound`.
    async fn tracked_revision(&self, path: &VaultPath) -> Result<RevisionToken, CoreError> {
        self.revisions
            .get_revision(path)
            .await
            .ok_or_else(|| CoreError::NoteNotFound(path.clone()))
    }

    /// Read a note's parsed content.
    pub async fn read(&self, path: &str) -> Result<Note, CoreError> {
        let vault_path = Self::validate_note_path(path)?;
        self.read_and_parse(&vault_path).await
    }

    /// Create a new note. Fails if a note already exists at the path.
    pub async fn create(&self, path: &str, content: &str) -> Result<(), CoreError> {
        let vault_path = Self::validate_note_path(path)?;

        let new_rev = self
            .storage
            .write(
                &vault_path,
                FileContent::Markdown(content.to_string()),
                None,
            )
            .await?;

        self.revisions.update_revision(&vault_path, new_rev).await;
        Ok(())
    }

    /// Update an existing note. Fails if the note is not tracked.
    ///
    /// Revision control is handled server-side via the `RevisionTracker`.
    pub async fn update(&self, path: &str, content: &str) -> Result<(), CoreError> {
        let vault_path = Self::validate_note_path(path)?;
        let revision = self.tracked_revision(&vault_path).await?;

        let new_rev = self
            .storage
            .write(
                &vault_path,
                FileContent::Markdown(content.to_string()),
                Some(revision),
            )
            .await?;

        self.revisions.update_revision(&vault_path, new_rev).await;
        Ok(())
    }

    /// Delete a note. Fails if the note is not tracked.
    pub async fn delete(&self, path: &str) -> Result<(), CoreError> {
        let vault_path = Self::validate_note_path(path)?;
        let revision = self.tracked_revision(&vault_path).await?;

        self.storage.delete(&vault_path, revision).await?;
        self.revisions.remove_revision(&vault_path).await;
        Ok(())
    }

    /// Rename/move a note. Fails if the source is not tracked.
    pub async fn rename(&self, from: &str, to: &str) -> Result<(), CoreError> {
        let from_path = Self::validate_note_path(from)?;
        let to_path = Self::validate_note_path(to)?;
        let revision = self.tracked_revision(&from_path).await?;

        self.storage.r#move(&from_path, &to_path, revision).await?;

        // Update tracker: remove old path, read new path's token
        self.revisions.remove_revision(&from_path).await;
        if let Ok(file) = self.storage.read(&to_path).await {
            self.revisions
                .update_revision(&to_path, file.meta.revision_token)
                .await;
        }

        Ok(())
    }

    /// List note paths under a folder.
    ///
    /// Uses the index for listing when available.
    pub async fn list(
        &self,
        folder: Option<&VaultPath>,
        recursive: bool,
    ) -> Result<Vec<VaultPath>, CoreError> {
        let results = self.index.list(folder, recursive).await?;
        let mut paths: Vec<VaultPath> = results.into_iter().map(|r| r.path).collect();
        paths.sort();
        Ok(paths)
    }

    /// Check if a note is tracked.
    pub async fn exists(&self, path: &str) -> Result<bool, CoreError> {
        let vault_path = Self::validate_note_path(path)?;
        Ok(self.revisions.get_revision(&vault_path).await.is_some())
    }

    /// Search for notes matching a query. Returns note-level results with scores.
    ///
    /// Thin wrapper over the index.
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

    /// Get tag entries, optionally filtered by prefix and folder.
    pub async fn tags(
        &self,
        prefix: Option<&str>,
        folder: Option<&VaultPath>,
    ) -> Result<Vec<TagEntry>, CoreError> {
        let results = self.index.list(folder, true).await?;
        let mut tag_map: HashMap<String, HashSet<VaultPath>> = HashMap::new();

        for result in &results {
            for tag in result.tags() {
                tag_map.entry(tag).or_default().insert(result.path.clone());
            }
        }

        let mut entries: Vec<TagEntry> = tag_map
            .into_iter()
            .filter(|(tag, _)| prefix.is_none_or(|p| tag.starts_with(p)))
            .map(|(tag, note_paths)| TagEntry {
                tag,
                count: note_paths.len(),
                children: Vec::new(),
                note_paths: note_paths.into_iter().collect(),
            })
            .collect();

        // Build parent-child relationships
        let all_tags: Vec<String> = entries.iter().map(|t| t.tag.clone()).collect();
        for entry in &mut entries {
            entry.children = find_direct_children(&entry.tag, &all_tags);
        }

        entries.sort_by(|a, b| a.tag.cmp(&b.tag));
        Ok(entries)
    }

    /// Get note paths that link to the given target.
    pub async fn backlinks(&self, target: &str) -> Result<Vec<VaultPath>, CoreError> {
        let results = self.index.backlinks(target).await?;
        Ok(results.into_iter().map(|r| r.path).collect())
    }

    /// Get all links from a note.
    pub async fn forward_links(&self, path: &str) -> Result<Vec<IndexLink>, CoreError> {
        let vault_path = Self::validate_note_path(path)?;
        let links = self.index.forward_links(&vault_path).await?;
        Ok(links)
    }

    /// Get the vault name.
    pub fn vault_name(&self) -> &str {
        &self.vault_name
    }
}

// Static methods that don't depend on generic parameters
impl<S, I, O, R> TarnCore<S, I, O, R> {
    // --- Note parsing (stateless) ---

    /// Parse markdown content into a Note. Stateless.
    pub fn parse(content: &str) -> Note {
        Note::from(content)
    }

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

    fn validate_note_path(path: &str) -> Result<VaultPath, CoreError> {
        let vault_path: VaultPath = path.try_into().map_err(StorageError::from)?;
        if !vault_path.is_note() {
            return Err(CoreError::NotMarkdown(vault_path));
        }
        Ok(vault_path)
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

    #[tokio::test]
    async fn test_create_and_read() {
        let (_dir, core) = setup();
        core.create("hello.md", "# Hello\nWorld").await.unwrap();

        let note = core.read("hello.md").await.unwrap();
        assert_eq!(note.to_string().trim(), "# Hello\nWorld");
    }

    #[tokio::test]
    async fn test_exists_reflects_tracker() {
        let (_dir, core) = setup();
        assert!(!core.exists("missing.md").await.unwrap());

        core.create("note.md", "content").await.unwrap();
        assert!(core.exists("note.md").await.unwrap());
    }

    #[tokio::test]
    async fn test_create_not_markdown() {
        let (_dir, core) = setup();
        let err = core.create("image.png", "data").await.unwrap_err();
        assert!(matches!(err, CoreError::NotMarkdown(_)));
    }

    #[tokio::test]
    async fn test_update_existing_note() {
        let (_dir, core) = setup();
        core.create("note.md", "original").await.unwrap();
        core.update("note.md", "updated").await.unwrap();

        let note = core.read("note.md").await.unwrap();
        assert_eq!(note.to_string().trim(), "updated");
    }

    #[tokio::test]
    async fn test_update_untracked_fails() {
        let (_dir, core) = setup();
        let err = core.update("note.md", "content").await.unwrap_err();
        assert!(matches!(err, CoreError::NoteNotFound(_)));
    }

    #[tokio::test]
    async fn test_create_existing_fails() {
        let (_dir, core) = setup();
        core.create("note.md", "v1").await.unwrap();
        let err = core.create("note.md", "v2").await.unwrap_err();
        assert!(matches!(
            err,
            CoreError::Storage(StorageError::FileAlreadyExists(_))
        ));
    }

    #[tokio::test]
    async fn test_delete() {
        let (_dir, core) = setup();
        core.create("note.md", "content").await.unwrap();
        core.delete("note.md").await.unwrap();
        assert!(!core.exists("note.md").await.unwrap());
    }

    #[tokio::test]
    async fn test_delete_untracked_fails() {
        let (_dir, core) = setup();
        let err = core.delete("note.md").await.unwrap_err();
        assert!(matches!(err, CoreError::NoteNotFound(_)));
    }

    #[tokio::test]
    async fn test_parse_and_resolve_section() {
        let note = TestCore::parse("# Top\n\ncontent\n\n## Sub\n\nsub content");
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
        core.create("note.md", "content").await.unwrap();
        core.rebuild_index().await.unwrap();

        let hits = core.search("", &[], &[], 10, None, 0.0).await.unwrap();
        assert!(hits.is_empty());
    }

    #[tokio::test]
    async fn test_search_finds_content() {
        let (_dir, core) = setup();
        core.create("note.md", "# Rust\n\nRust is a systems language")
            .await
            .unwrap();
        core.rebuild_index().await.unwrap();

        let hits = core
            .search("systems", &[], &[], 10, None, 0.0)
            .await
            .unwrap();
        assert!(!hits.is_empty());
        assert!(hits[0].path.to_string().starts_with("note.md"));
    }

    #[tokio::test]
    async fn test_tags() {
        let (_dir, core) = setup();
        core.create(
            "note.md",
            "---\ntags:\n  - rust\n  - programming\n---\n# Note",
        )
        .await
        .unwrap();
        core.rebuild_index().await.unwrap();

        let entries = core.tags(None, None).await.unwrap();
        let tag_names: Vec<&str> = entries.iter().map(|e| e.tag.as_str()).collect();
        assert!(tag_names.contains(&"rust"));
        assert!(tag_names.contains(&"programming"));
    }

    #[tokio::test]
    async fn test_list() {
        let (_dir, core) = setup();
        core.create("a.md", "# A").await.unwrap();
        core.create("b.md", "# B").await.unwrap();
        core.rebuild_index().await.unwrap();

        let paths = core.list(None, true).await.unwrap();
        assert_eq!(paths.len(), 2);
    }

    #[tokio::test]
    async fn test_vault_name() {
        let (_dir, core) = setup();
        assert!(!core.vault_name().is_empty());
    }

    #[tokio::test]
    async fn test_rename() {
        let (_dir, core) = setup();
        core.create("old.md", "# Old").await.unwrap();

        core.rename("old.md", "new.md").await.unwrap();

        assert!(!core.exists("old.md").await.unwrap());
        assert!(core.exists("new.md").await.unwrap());
    }

    #[tokio::test]
    async fn test_rename_untracked_fails() {
        let (_dir, core) = setup();
        let err = core.rename("old.md", "new.md").await.unwrap_err();
        assert!(matches!(err, CoreError::NoteNotFound(_)));
    }

    #[tokio::test]
    async fn test_backlinks() {
        let (_dir, core) = setup();
        core.create("source.md", "See [[target]] for details.")
            .await
            .unwrap();
        core.create("target.md", "# Target").await.unwrap();
        core.rebuild_index().await.unwrap();

        let backlinks = core.backlinks("target").await.unwrap();
        assert_eq!(backlinks.len(), 1);
        assert_eq!(backlinks[0].to_string(), "source.md");
    }

    #[tokio::test]
    async fn test_forward_links() {
        let (_dir, core) = setup();
        core.create("note.md", "Links to [[alpha]] and [[beta|B]].")
            .await
            .unwrap();
        core.rebuild_index().await.unwrap();

        let links = core.forward_links("note.md").await.unwrap();
        assert_eq!(links.len(), 2);
    }

    #[tokio::test]
    async fn test_read_non_markdown_fails() {
        let (_dir, core) = setup();
        let err = core.read("image.png").await.unwrap_err();
        assert!(matches!(err, CoreError::NotMarkdown(_)));
    }

    // --- Revision tracking tests ---

    #[tokio::test]
    async fn test_create_updates_revision_tracker() {
        let (_dir, core) = setup();
        core.create("note.md", "content").await.unwrap();

        let path: VaultPath = "note.md".try_into().unwrap();
        assert!(core.revisions.get_revision(&path).await.is_some());
    }

    #[tokio::test]
    async fn test_update_updates_revision_tracker() {
        let (_dir, core) = setup();
        core.create("note.md", "v1").await.unwrap();
        let path: VaultPath = "note.md".try_into().unwrap();
        let rev1 = core.revisions.get_revision(&path).await.unwrap();

        core.update("note.md", "v2").await.unwrap();
        let rev2 = core.revisions.get_revision(&path).await.unwrap();
        assert_ne!(rev1, rev2);
    }

    #[tokio::test]
    async fn test_delete_removes_from_revision_tracker() {
        let (_dir, core) = setup();
        core.create("note.md", "content").await.unwrap();
        core.delete("note.md").await.unwrap();

        let path: VaultPath = "note.md".try_into().unwrap();
        assert!(core.revisions.get_revision(&path).await.is_none());
    }

    #[tokio::test]
    async fn test_rename_updates_revision_tracker() {
        let (_dir, core) = setup();
        core.create("old.md", "# Old").await.unwrap();
        core.rename("old.md", "new.md").await.unwrap();

        let old_path: VaultPath = "old.md".try_into().unwrap();
        let new_path: VaultPath = "new.md".try_into().unwrap();
        assert!(core.revisions.get_revision(&old_path).await.is_none());
        assert!(core.revisions.get_revision(&new_path).await.is_some());
    }

    #[tokio::test]
    async fn test_validate_revisions_updates_mismatches() {
        let (_dir, core) = setup();
        core.create("note.md", "v1").await.unwrap();
        let vault_path: VaultPath = "note.md".try_into().unwrap();
        let rev = core.revisions.get_revision(&vault_path).await.unwrap();

        // Modify file directly via storage to create mismatch
        let _new_rev = core
            .storage
            .write(
                &vault_path,
                FileContent::Markdown("v2".to_string()),
                Some(rev.clone()),
            )
            .await
            .unwrap();

        // Tracker still has old revision
        assert_eq!(
            core.revisions.get_revision(&vault_path).await,
            Some(rev.clone())
        );

        // Validate should fix the mismatch
        core.validate_revisions().await.unwrap();
        let tracked = core.revisions.get_revision(&vault_path).await.unwrap();
        assert_ne!(tracked, rev);
    }

    #[tokio::test]
    async fn test_validate_revisions_removes_deleted() {
        let (_dir, core) = setup();
        core.create("note.md", "content").await.unwrap();
        let vault_path: VaultPath = "note.md".try_into().unwrap();
        let rev = core.revisions.get_revision(&vault_path).await.unwrap();

        // Delete directly via storage
        core.storage.delete(&vault_path, rev).await.unwrap();

        // Tracker still has the entry
        assert!(core.revisions.get_revision(&vault_path).await.is_some());

        // Validate should remove it
        core.validate_revisions().await.unwrap();
        assert!(core.revisions.get_revision(&vault_path).await.is_none());
    }
}
