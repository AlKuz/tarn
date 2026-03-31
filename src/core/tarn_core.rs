use serde::{Serialize, de::DeserializeOwned};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use thiserror::Error;
use tokio::task::JoinHandle;
use tokio_stream::StreamExt;
use tracing::{debug, info, warn};

use crate::common::{Configurable, RevisionToken, VaultPath};
use crate::core::config::TarnConfig;
use crate::core::responses::{CoreSearchResponse, SearchHit, SearchOptions, TagEntry};
use crate::index::find_direct_children;
use crate::index::{Index, IndexError, IndexLink, SearchParams};
use crate::note_handler::{Note, Section};
use crate::observer::{Observer, ObserverError, StorageEvent};
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

pub struct TarnCore<S, I, O> {
    storage: Arc<S>,
    vault_name: String,
    index: Arc<I>,
    observer: Arc<O>,
}

impl<S, I, O> TarnCore<S, I, O> {
    pub fn new(storage: Arc<S>, vault_name: String, index: Arc<I>, observer: Arc<O>) -> Self {
        Self {
            storage,
            vault_name,
            index,
            observer,
        }
    }
}

impl<S, I, O> Configurable for TarnCore<S, I, O>
where
    S: Storage + Configurable + Send + Sync + 'static,
    I: Index + Send + Sync + 'static,
    O: Observer + Configurable + Send + Sync + 'static,
    S::Config: Serialize + DeserializeOwned,
    O::Config: Serialize + DeserializeOwned,
{
    type Config = TarnConfig<S::Config, <I as Configurable>::Config, O::Config>;

    fn config(&self) -> Self::Config {
        TarnConfig {
            vault_name: self.vault_name.clone(),
            storage: self.storage.config(),
            index: self.index.config(),
            observer: self.observer.config(),
        }
    }
}

impl<S, I, O> TarnCore<S, I, O>
where
    S: Storage + Send + Sync + 'static,
    I: Index + Send + Sync + 'static,
    O: Observer + Send + Sync + 'static,
{
    async fn read_and_parse(&self, path: &VaultPath) -> Result<(Note, RevisionToken), CoreError> {
        let file = self.storage.read(path).await?;
        match file.content {
            FileContent::Markdown(content) => {
                let mut note = Note::from(content.as_str());
                note.path = Some(path.clone());
                Ok((note, file.meta.revision_token))
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
                Ok((note, _)) => notes.push(note),
                Err(e) => {
                    warn!(path = %file_path, error = %e, "skipping note during index rebuild");
                }
            }
        }

        index.update_bulk(&notes).await?;
        Ok(())
    }

    /// Start background index synchronization.
    ///
    /// Spawns a task that watches for file changes and updates the index.
    /// Returns a handle to the background task.
    pub fn start_index_sync(&self) -> JoinHandle<()> {
        let index = self.index.clone();
        let storage = self.storage.clone();
        let observer = self.observer.clone();

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
                    StorageEvent::Created { path, .. } | StorageEvent::Updated { path, .. } => {
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

    /// Read a note's parsed content and revision token.
    pub async fn read(&self, path: &str) -> Result<(Note, RevisionToken), CoreError> {
        let vault_path = Self::validate_note_path(path)?;
        self.read_and_parse(&vault_path).await
    }

    /// Write content to a note path.
    ///
    /// Pass `None` for revision to create a new file (no conflict check).
    /// Pass `Some(revision)` to update with optimistic concurrency.
    pub async fn write(
        &self,
        path: &str,
        content: &str,
        revision: Option<RevisionToken>,
    ) -> Result<RevisionToken, CoreError> {
        let vault_path = Self::validate_note_path(path)?;
        let rev = self
            .storage
            .write(
                &vault_path,
                FileContent::Markdown(content.to_string()),
                revision,
            )
            .await?;
        Ok(rev)
    }

    /// Delete a note with conflict check.
    pub async fn delete(&self, path: &str, revision: RevisionToken) -> Result<(), CoreError> {
        let vault_path = Self::validate_note_path(path)?;
        self.storage.delete(&vault_path, revision).await?;
        Ok(())
    }

    /// Rename/move a note with conflict check.
    pub async fn rename(
        &self,
        from: &str,
        to: &str,
        revision: RevisionToken,
    ) -> Result<(), CoreError> {
        let from_path = Self::validate_note_path(from)?;
        let to_path = Self::validate_note_path(to)?;
        self.storage.r#move(&from_path, &to_path, revision).await?;
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
        let sections = self.index.list(folder, recursive).await?;
        let mut seen = HashSet::new();
        let mut paths = Vec::new();
        for section in sections {
            if seen.insert(section.note_path.clone()) {
                paths.push(section.note_path);
            }
        }
        paths.sort();
        Ok(paths)
    }

    /// Check if a note exists. Returns its revision token if found.
    pub async fn exists(&self, path: &str) -> Result<Option<RevisionToken>, CoreError> {
        let vault_path = Self::validate_note_path(path)?;
        if self.storage.exists(&vault_path).await? {
            let file = self.storage.read(&vault_path).await?;
            Ok(Some(file.meta.revision_token))
        } else {
            Ok(None)
        }
    }

    /// Search for notes matching a query. Returns deduplicated hits with scores.
    ///
    /// Returns empty results when query is empty — use `list()` for listing.
    pub async fn search(
        &self,
        query: &str,
        options: SearchOptions,
    ) -> Result<CoreSearchResponse, CoreError> {
        if query.is_empty() {
            return Ok(CoreSearchResponse {
                hits: Vec::new(),
                total: 0,
            });
        }

        // Fetch sections from the index with adaptive sizing. Sections
        // deduplicate to fewer notes, so we start at 4x and double until we
        // have enough unique notes or the index is exhausted.
        let target = options.limit + options.offset;
        let mut multiplier = 4;
        let mut note_hits: HashMap<VaultPath, (f32, Vec<Vec<String>>)>;

        loop {
            let index_limit = target * multiplier;
            let params = SearchParams {
                folder: options.folder.clone(),
                tags: options.tags.clone(),
                limit: index_limit,
                offset: 0,
            };

            let search_results = self.index.search(query, params).await?;
            let result_count = search_results.len();

            // Deduplicate by note path, collect matching sections per note
            note_hits = HashMap::new();
            for (section, score) in search_results {
                let entry = note_hits
                    .entry(section.note_path.clone())
                    .or_insert_with(|| (0.0, Vec::new()));
                if score > entry.0 {
                    entry.0 = score;
                }
                entry.1.push(section.heading_path);
            }

            // Enough unique notes, or index exhausted
            if note_hits.len() >= target || result_count < index_limit {
                break;
            }
            multiplier *= 2;
        }

        let mut hits: Vec<SearchHit> = note_hits
            .into_iter()
            .map(|(path, (score, sections))| SearchHit {
                path,
                score,
                sections,
            })
            .collect();

        // Sort by score descending
        hits.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let total = hits.len();
        let hits: Vec<SearchHit> = hits
            .into_iter()
            .skip(options.offset)
            .take(options.limit)
            .collect();

        Ok(CoreSearchResponse { hits, total })
    }

    /// Get tag entries, optionally filtered by prefix and folder.
    pub async fn tags(
        &self,
        prefix: Option<&str>,
        folder: Option<&VaultPath>,
    ) -> Result<Vec<TagEntry>, CoreError> {
        let sections = self.index.list(folder, true).await?;
        let mut tag_map: HashMap<String, HashSet<VaultPath>> = HashMap::new();

        for section in &sections {
            for tag in &section.tags {
                tag_map
                    .entry(tag.clone())
                    .or_default()
                    .insert(section.note_path.clone());
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
        let sections = self.index.backlinks(target).await?;
        let mut seen = HashSet::new();
        let mut paths = Vec::new();
        for section in sections {
            if seen.insert(section.note_path.clone()) {
                paths.push(section.note_path);
            }
        }
        Ok(paths)
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
impl<S, I, O> TarnCore<S, I, O> {
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
    >;

    fn setup() -> (TempDir, TestCore) {
        let dir = TempDir::new().unwrap();
        let core = TarnConfig::local(dir.path().to_path_buf()).build().unwrap();
        (dir, core)
    }

    #[tokio::test]
    async fn test_write_and_read() {
        let (_dir, core) = setup();
        let rev = core
            .write("hello.md", "# Hello\nWorld", None)
            .await
            .unwrap();
        assert!(!rev.to_string().is_empty());

        let (note, read_rev) = core.read("hello.md").await.unwrap();
        assert_eq!(note.to_string().trim(), "# Hello\nWorld");
        assert_eq!(read_rev, rev);
    }

    #[tokio::test]
    async fn test_exists_returns_revision() {
        let (_dir, core) = setup();
        assert!(core.exists("missing.md").await.unwrap().is_none());

        let rev = core.write("note.md", "content", None).await.unwrap();
        let exists_rev = core.exists("note.md").await.unwrap();
        assert_eq!(exists_rev, Some(rev));
    }

    #[tokio::test]
    async fn test_write_not_markdown() {
        let (_dir, core) = setup();
        let err = core.write("image.png", "data", None).await.unwrap_err();
        assert!(matches!(err, CoreError::NotMarkdown(_)));
    }

    #[tokio::test]
    async fn test_write_update_with_revision() {
        let (_dir, core) = setup();
        let rev1 = core.write("note.md", "original", None).await.unwrap();
        let rev2 = core.write("note.md", "updated", Some(rev1)).await.unwrap();
        assert_ne!(rev2, RevisionToken::from(""));

        let (note, _) = core.read("note.md").await.unwrap();
        assert_eq!(note.to_string().trim(), "updated");
    }

    #[tokio::test]
    async fn test_write_conflict() {
        let (_dir, core) = setup();
        let rev1 = core.write("note.md", "original", None).await.unwrap();
        core.write("note.md", "v2", Some(rev1.clone()))
            .await
            .unwrap();
        // Stale revision
        let err = core.write("note.md", "v3", Some(rev1)).await.unwrap_err();
        assert!(matches!(
            err,
            CoreError::Storage(StorageError::Conflict(_, _, _))
        ));
    }

    #[tokio::test]
    async fn test_delete() {
        let (_dir, core) = setup();
        let rev = core.write("note.md", "content", None).await.unwrap();
        core.delete("note.md", rev).await.unwrap();
        assert!(core.exists("note.md").await.unwrap().is_none());
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
        core.write("note.md", "content", None).await.unwrap();
        core.rebuild_index().await.unwrap();

        let result = core
            .search(
                "",
                SearchOptions {
                    limit: 10,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(result.total, 0);
    }

    #[tokio::test]
    async fn test_search_finds_content() {
        let (_dir, core) = setup();
        core.write("note.md", "# Rust\n\nRust is a systems language", None)
            .await
            .unwrap();
        core.rebuild_index().await.unwrap();

        let result = core
            .search(
                "systems",
                SearchOptions {
                    limit: 10,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(result.total, 1);
        assert_eq!(result.hits[0].path.to_string(), "note.md");
    }

    #[tokio::test]
    async fn test_tags() {
        let (_dir, core) = setup();
        core.write(
            "note.md",
            "---\ntags:\n  - rust\n  - programming\n---\n# Note",
            None,
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
        core.write("a.md", "# A", None).await.unwrap();
        core.write("b.md", "# B", None).await.unwrap();
        core.rebuild_index().await.unwrap();

        let paths = core.list(None, true).await.unwrap();
        assert_eq!(paths.len(), 2);
    }

    #[tokio::test]
    async fn test_vault_name() {
        let (_dir, core) = setup();
        assert!(!core.vault_name().is_empty());
    }
}
