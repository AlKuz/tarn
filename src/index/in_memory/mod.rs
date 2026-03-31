//! In-memory index implementation with BM25 search and tag filtering.

mod bm25;
mod tags;

pub use bm25::BM25Index;
pub use tags::TagIndex;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::RwLock;

use crate::common::{Configurable, VaultPath};
use crate::note_handler::Note;
use crate::tokenizer::{Tokenizer, TokenizerConfig};

use super::{Index, IndexConfig, IndexError, IndexLink, IndexMeta, SearchParams, SectionEntry};

// ---------------------------------------------------------------------------
// InMemoryIndex errors
// ---------------------------------------------------------------------------

/// Errors specific to in-memory index operations.
#[derive(Debug, Error)]
pub enum InMemoryIndexError {
    #[error("tokenizer error: {0}")]
    Tokenizer(#[from] crate::tokenizer::TokenizerError),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

impl From<InMemoryIndexError> for IndexError {
    fn from(e: InMemoryIndexError) -> Self {
        IndexError::Backend(e.to_string())
    }
}

// ---------------------------------------------------------------------------
// IndexData
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize)]
struct IndexData {
    #[serde(default)]
    version: u32,
    meta: IndexMeta,
    sections: HashMap<VaultPath, SectionEntry>,
    tag_index: TagIndex,
    bm25_index: BM25Index,
}

impl IndexData {
    fn new(tokenizer_config: TokenizerConfig) -> Self {
        Self {
            version: 0,
            meta: IndexMeta {
                note_count: 0,
                last_indexed: None,
                tokenizer_config,
            },
            sections: HashMap::new(),
            tag_index: TagIndex::new(),
            bm25_index: BM25Index::new(),
        }
    }

    /// Check if a note has any sections in the index.
    fn has_note(&self, note_path: &VaultPath) -> bool {
        self.sections.values().any(|e| &e.note_path == note_path)
    }
}

// ---------------------------------------------------------------------------
// InMemoryIndex
// ---------------------------------------------------------------------------

/// In-memory index with optional persistence.
///
/// Uses `tokio::sync::RwLock` for thread-safe async access. Supports:
/// - BM25 full-text search with pluggable tokenizers
/// - Tag-based filtering with include/exclude
/// - Auto-persistence after mutations
pub struct InMemoryIndex {
    data: RwLock<IndexData>,
    tokenizer: Box<dyn Tokenizer>,
    persistence_path: Option<PathBuf>,
}

impl InMemoryIndex {
    /// Create an index with the given tokenizer and optional persistence path.
    ///
    /// If a persistence path is provided and exists, the index is loaded from disk.
    /// The tokenizer config is checked against the persisted config — if they differ,
    /// the persisted data is discarded and a fresh index is created.
    pub fn new(
        tokenizer: Box<dyn Tokenizer>,
        persistence_path: Option<PathBuf>,
    ) -> Result<Self, InMemoryIndexError> {
        let tokenizer_config = tokenizer.config();
        let inner = match &persistence_path {
            Some(path) if path.exists() => {
                let data = std::fs::read(path)?;
                let index_data: IndexData = serde_json::from_slice(&data)?;
                if index_data.meta.tokenizer_config == tokenizer_config {
                    index_data
                } else {
                    tracing::info!("tokenizer config changed, rebuilding index");
                    IndexData::new(tokenizer_config)
                }
            }
            _ => IndexData::new(tokenizer_config),
        };
        Ok(Self {
            data: RwLock::new(inner),
            tokenizer,
            persistence_path,
        })
    }

    /// Save the index to disk (atomic write via temp file + rename).
    async fn persist(&self) -> Result<(), InMemoryIndexError> {
        let Some(path) = &self.persistence_path else {
            return Ok(());
        };

        let data = {
            let inner = self.data.read().await;
            serde_json::to_vec(&*inner)?
        };

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // Atomic write: temp file + rename
        let temp_path = path.with_extension("tmp");
        tokio::fs::write(&temp_path, &data).await?;
        tokio::fs::rename(&temp_path, path).await?;

        Ok(())
    }

    /// Build a section VaultPath from a note path and heading path.
    fn make_section_path(
        note_path: &VaultPath,
        heading_path: &[String],
    ) -> Result<VaultPath, IndexError> {
        let section_path_str = heading_path.join("/");
        VaultPath::new(format!("{}#{}", note_path.as_str(), section_path_str)).map_err(|e| {
            IndexError::Backend(format!("invalid section path for {}: {}", note_path, e))
        })
    }

    /// Index a note, extracting all sections. Increments note_count if new.
    fn index_note(
        inner: &mut IndexData,
        tokenizer: &dyn Tokenizer,
        note: &Note,
        note_path: &VaultPath,
    ) {
        let is_new = !inner.has_note(note_path);

        // Get frontmatter tags (attached to all sections)
        let frontmatter_tags: Vec<String> = note
            .frontmatter
            .as_ref()
            .map(|fm| fm.tags.clone())
            .unwrap_or_default();

        for section in &note.sections {
            let section_path = match Self::make_section_path(note_path, &section.heading_path) {
                Ok(p) => p,
                Err(e) => {
                    tracing::warn!(note = %note_path, error = %e, "skipping section with invalid path");
                    continue;
                }
            };

            // Combine frontmatter tags with inline tags
            let mut all_tags: Vec<String> = frontmatter_tags.clone();
            all_tags.extend(section.tags.iter().map(|t| t.name().to_string()));

            // Convert links to IndexLink
            let links: Vec<IndexLink> = section.links.iter().map(IndexLink::from).collect();
            let tokens = tokenizer.tokenize(&section.content);

            let entry = SectionEntry {
                note_path: note_path.clone(),
                heading_path: section.heading_path.clone(),
                tags: all_tags.clone(),
                links,
                token_count: tokens.len(),
                revision: crate::common::RevisionToken::from(chrono::Utc::now().to_rfc3339()),
            };

            // Add to BM25 index
            inner.bm25_index.add_document(section_path.clone(), &tokens);

            // Add to tag index
            inner.tag_index.add(section_path.clone(), &all_tags);

            // Store section entry
            inner.sections.insert(section_path, entry);
        }

        if is_new {
            inner.meta.note_count += 1;
        }
    }

    /// Remove all sections for a note path. Decrements note_count if removed.
    fn remove_note_sections(inner: &mut IndexData, note_path: &VaultPath) {
        let section_paths: Vec<VaultPath> = inner
            .sections
            .keys()
            .filter(|path| path.note_path().as_ref() == Some(note_path))
            .cloned()
            .collect();

        if !section_paths.is_empty() {
            for section_path in section_paths {
                inner.sections.remove(&section_path);
                inner.tag_index.remove(&section_path);
                inner.bm25_index.remove_document(&section_path);
            }
            inner.meta.note_count = inner.meta.note_count.saturating_sub(1);
        }
    }
}

impl Configurable for InMemoryIndex {
    type Config = IndexConfig;

    fn config(&self) -> Self::Config {
        IndexConfig::InMemory {
            tokenizer: self.tokenizer.config(),
            persistence_path: self.persistence_path.clone(),
        }
    }
}

impl Index for InMemoryIndex {
    async fn get(&self, path: &VaultPath) -> Result<Vec<SectionEntry>, IndexError> {
        let inner = self.data.read().await;

        let sections: Vec<SectionEntry> = inner
            .sections
            .iter()
            .filter(|(key, _)| key.note_path().as_ref() == Some(path))
            .map(|(_, entry)| entry.clone())
            .collect();

        if sections.is_empty() {
            return Err(IndexError::NotFound(path.clone()));
        }

        Ok(sections)
    }

    async fn update(&self, note: &Note) -> Result<(), IndexError> {
        let Some(note_path) = &note.path else {
            return Err(IndexError::Backend("note has no path".to_string()));
        };

        {
            let mut inner = self.data.write().await;

            // Remove existing sections for this note
            Self::remove_note_sections(&mut inner, note_path);

            // Index the note
            Self::index_note(&mut inner, &*self.tokenizer, note, note_path);

            inner.meta.last_indexed = Some(chrono::Utc::now());
        }

        self.persist().await.map_err(Into::into)
    }

    async fn remove(&self, path: &VaultPath) -> Result<(), IndexError> {
        {
            let mut inner = self.data.write().await;
            Self::remove_note_sections(&mut inner, path);
        }

        self.persist().await.map_err(Into::into)
    }

    async fn search(
        &self,
        query: &str,
        params: SearchParams,
    ) -> Result<Vec<(SectionEntry, f32)>, IndexError> {
        let inner = self.data.read().await;

        // Build tag filters from params
        let include_tags: Option<HashSet<String>> =
            params.tags.as_ref().map(|t| t.iter().cloned().collect());

        // Get BM25 search results
        let query_tokens = self.tokenizer.tokenize(query);
        let bm25_limit = params.limit + params.offset + 1000; // Get extra for filtering
        let bm25_results = inner.bm25_index.search(&query_tokens, bm25_limit);

        // Filter results
        let results: Vec<(SectionEntry, f32)> = bm25_results
            .into_iter()
            .filter_map(|(section_path, score)| {
                let entry = inner.sections.get(&section_path)?;

                // Apply folder filter
                if let Some(folder) = &params.folder
                    && !entry.note_path.is_under_folder(folder)
                {
                    return None;
                }

                // Apply tag filter (section must have at least one of the tags)
                if let Some(tags) = &include_tags {
                    let section_tags: HashSet<String> = entry.tags.iter().cloned().collect();
                    if section_tags.is_disjoint(tags) {
                        return None;
                    }
                }

                Some((entry.clone(), score))
            })
            .skip(params.offset)
            .take(params.limit)
            .collect();

        Ok(results)
    }

    async fn list(
        &self,
        folder: Option<&VaultPath>,
        recursive: bool,
    ) -> Result<Vec<SectionEntry>, IndexError> {
        let inner = self.data.read().await;

        let sections: Vec<SectionEntry> = inner
            .sections
            .values()
            .filter(|entry| match folder {
                None => true,
                Some(f) if recursive => entry.note_path.is_under_folder(f),
                Some(f) => entry.note_path.is_in_folder(f),
            })
            .cloned()
            .collect();

        Ok(sections)
    }

    async fn backlinks(&self, target: &str) -> Result<Vec<SectionEntry>, IndexError> {
        let inner = self.data.read().await;

        let sections: Vec<SectionEntry> = inner
            .sections
            .values()
            .filter(|entry| {
                entry.links.iter().any(|link| match link {
                    IndexLink::Wiki { target: t, .. } => t == target,
                    _ => false,
                })
            })
            .cloned()
            .collect();

        Ok(sections)
    }

    async fn forward_links(&self, path: &VaultPath) -> Result<Vec<IndexLink>, IndexError> {
        let inner = self.data.read().await;

        let links: Vec<IndexLink> = inner
            .sections
            .values()
            .filter(|entry| &entry.note_path == path)
            .flat_map(|entry| entry.links.iter().cloned())
            .collect();

        Ok(links)
    }

    async fn meta(&self) -> Result<IndexMeta, IndexError> {
        let inner = self.data.read().await;
        Ok(inner.meta.clone())
    }

    async fn set_meta(&self, meta: IndexMeta) -> Result<(), IndexError> {
        {
            let mut inner = self.data.write().await;
            inner.meta = meta;
        }

        self.persist().await.map_err(Into::into)
    }

    async fn clear(&self) -> Result<(), IndexError> {
        {
            let mut inner = self.data.write().await;
            inner.sections.clear();
            inner.tag_index.clear();
            inner.bm25_index.clear();
            inner.meta.note_count = 0;
        }

        self.persist().await.map_err(Into::into)
    }

    async fn count(&self) -> Result<usize, IndexError> {
        let inner = self.data.read().await;
        Ok(inner.sections.len())
    }

    async fn update_bulk(&self, notes: &[Note]) -> Result<(), IndexError> {
        {
            let mut inner = self.data.write().await;

            for note in notes {
                let Some(note_path) = &note.path else {
                    continue;
                };

                Self::remove_note_sections(&mut inner, note_path);
                Self::index_note(&mut inner, &*self.tokenizer, note, note_path);
            }

            inner.meta.last_indexed = Some(chrono::Utc::now());
        }

        self.persist().await.map_err(Into::into)
    }

    async fn remove_bulk(&self, paths: &[VaultPath]) -> Result<(), IndexError> {
        {
            let mut inner = self.data.write().await;

            for path in paths {
                Self::remove_note_sections(&mut inner, path);
            }
        }

        self.persist().await.map_err(Into::into)
    }
}

// ---------------------------------------------------------------------------
// Link conversion
// ---------------------------------------------------------------------------

impl From<&crate::note_handler::Link> for IndexLink {
    fn from(link: &crate::note_handler::Link) -> Self {
        use crate::note_handler::Link;
        match link {
            Link::Wiki(w) => IndexLink::Wiki {
                target: w.target.clone(),
                alias: w.alias.clone(),
            },
            Link::Markdown(m) => IndexLink::Markdown {
                url: m.url.clone(),
                text: m.text.clone(),
            },
            Link::Url(u) => IndexLink::Url { url: u.url.clone() },
            Link::Email(e) => IndexLink::Email {
                address: e.address.clone(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::Buildable;

    fn make_index() -> InMemoryIndex {
        IndexConfig::InMemory {
            tokenizer: TokenizerConfig::default(),
            persistence_path: None,
        }
        .build()
        .unwrap()
    }

    fn make_note(path: &str, content: &str) -> Note {
        let mut note = Note::from(content);
        note.path = Some(VaultPath::new(path).unwrap());
        note
    }

    #[test]
    fn section_path_construction() {
        let path = VaultPath::new("projects/alpha.md").unwrap();

        // Root section
        let sp = InMemoryIndex::make_section_path(&path, &[]).unwrap();
        assert_eq!(sp.as_str(), "projects/alpha.md#");
        assert_eq!(sp.note_path(), Some(path.clone()));
        assert!(sp.section_headings().is_empty());

        // Single heading
        let sp = InMemoryIndex::make_section_path(&path, &["Goals".to_string()]).unwrap();
        assert_eq!(sp.as_str(), "projects/alpha.md#Goals");
        assert_eq!(sp.section_headings(), vec!["Goals"]);

        // Nested headings
        let sp = InMemoryIndex::make_section_path(&path, &["Goals".to_string(), "Q1".to_string()])
            .unwrap();
        assert_eq!(sp.as_str(), "projects/alpha.md#Goals/Q1");
        assert_eq!(sp.section_headings(), vec!["Goals", "Q1"]);
    }

    #[tokio::test]
    async fn index_and_search() {
        let index = make_index();

        let note1 = make_note(
            "rust-guide.md",
            "# Introduction\n\nRust is a systems programming language.\n\n## Safety\n\nMemory safety without garbage collection.",
        );
        let note2 = make_note(
            "python-guide.md",
            "# Python\n\nPython is a dynamic language.\n",
        );

        index.update(&note1).await.unwrap();
        index.update(&note2).await.unwrap();

        let results = index
            .search(
                "programming language",
                SearchParams {
                    limit: 10,
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        assert!(!results.is_empty());
    }

    #[tokio::test]
    async fn get_sections_for_note() {
        let index = make_index();

        let note = make_note(
            "note.md",
            "# First\n\nContent one.\n\n## Second\n\nContent two.\n",
        );
        index.update(&note).await.unwrap();

        let path = VaultPath::new("note.md").unwrap();
        let sections = index.get(&path).await.unwrap();

        // Should have 2 sections: # First, ## Second (no empty root when content starts with heading)
        assert_eq!(sections.len(), 2);
    }

    #[tokio::test]
    async fn remove_note() {
        let index = make_index();

        let note = make_note("note.md", "# Test\n\nContent.\n");
        index.update(&note).await.unwrap();

        let path = VaultPath::new("note.md").unwrap();
        assert!(index.get(&path).await.is_ok());

        index.remove(&path).await.unwrap();
        assert!(matches!(
            index.get(&path).await,
            Err(IndexError::NotFound(_))
        ));
    }

    #[tokio::test]
    async fn clear_index() {
        let index = make_index();

        let note = make_note("note.md", "Content.\n");
        index.update(&note).await.unwrap();

        assert_eq!(index.count().await.unwrap(), 1);

        index.clear().await.unwrap();

        assert_eq!(index.count().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn search_with_folder_filter() {
        let index = make_index();

        let note1 = make_note(
            "projects/alpha/design.md",
            "# Design\n\nProject design notes.\n",
        );
        let note2 = make_note(
            "daily/2024-01-01.md",
            "# Daily\n\nDaily notes about project.\n",
        );

        index.update(&note1).await.unwrap();
        index.update(&note2).await.unwrap();

        let results = index
            .search(
                "project",
                SearchParams {
                    folder: Some(VaultPath::new("projects/").unwrap()),
                    limit: 10,
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        assert!(!results.is_empty());
        assert!(
            results
                .iter()
                .all(|(e, _)| e.note_path.as_str().starts_with("projects/"))
        );
    }

    #[tokio::test]
    async fn backlinks() {
        let index = make_index();

        let note1 = make_note("note1.md", "Check [[note2]] for details.\n");
        let note2 = make_note("note2.md", "Reference content.\n");

        index.update(&note1).await.unwrap();
        index.update(&note2).await.unwrap();

        let backlinks = index.backlinks("note2").await.unwrap();

        assert_eq!(backlinks.len(), 1);
        assert_eq!(backlinks[0].note_path.as_str(), "note1.md");
    }

    #[tokio::test]
    async fn forward_links() {
        let index = make_index();

        let note = make_note("note.md", "See [[target1]] and [[target2|alias]].\n");
        index.update(&note).await.unwrap();

        let path = VaultPath::new("note.md").unwrap();
        let links = index.forward_links(&path).await.unwrap();

        assert_eq!(links.len(), 2);
    }

    #[tokio::test]
    async fn persistence_same_tokenizer_preserves_data() {
        let dir = tempfile::tempdir().unwrap();
        let index_path = dir.path().join("index.json");

        let make_config = || IndexConfig::InMemory {
            tokenizer: TokenizerConfig::default(),
            persistence_path: Some(index_path.clone()),
        };

        // Create index, add data, persist
        let index = make_config().build().unwrap();
        let note = make_note("note.md", "# Test\n\nRust programming.\n");
        index.update(&note).await.unwrap();
        assert_eq!(index.count().await.unwrap(), 1);
        drop(index);

        // Reload with same config — data should be preserved
        let index = make_config().build().unwrap();
        assert_eq!(index.count().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn persistence_different_tokenizer_rebuilds_index() {
        let dir = tempfile::tempdir().unwrap();
        let index_path = dir.path().join("index.json");

        // Create index with Naive tokenizer, add data, persist
        let config = IndexConfig::InMemory {
            tokenizer: TokenizerConfig::default(),
            persistence_path: Some(index_path.clone()),
        };
        let index = config.build().unwrap();
        let note = make_note("note.md", "# Test\n\nRust programming.\n");
        index.update(&note).await.unwrap();
        assert_eq!(index.count().await.unwrap(), 1);
        drop(index);

        // Tamper the persisted meta to simulate a different tokenizer config
        let data = std::fs::read(&index_path).unwrap();
        let mut snapshot: serde_json::Value = serde_json::from_slice(&data).unwrap();
        snapshot["meta"]["tokenizer_config"] = serde_json::json!({
            "type": "hugging_face",
            "model_id": "bert-base-uncased"
        });
        std::fs::write(&index_path, serde_json::to_vec(&snapshot).unwrap()).unwrap();

        // Reload with Naive — snapshot has HuggingFace, so mismatch → fresh index
        let config = IndexConfig::InMemory {
            tokenizer: TokenizerConfig::default(),
            persistence_path: Some(index_path.clone()),
        };
        let index = config.build().unwrap();
        assert_eq!(index.count().await.unwrap(), 0);
    }
}
