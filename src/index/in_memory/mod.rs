//! In-memory index implementation with BM25 search and tag filtering.

mod bm25;
mod tags;

pub use bm25::{BM25Error, BM25Index};
pub use tags::TagIndex;

use std::collections::HashSet;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::RwLock;

use crate::common::VaultPath;
use crate::note::Note;

use super::{Index, IndexError, IndexLink, IndexMeta, SearchParams, SectionEntry};

// ---------------------------------------------------------------------------
// SectionId
// ---------------------------------------------------------------------------

/// Natural ID for a section: `{note_path}#{heading1/heading2/heading3}`
///
/// Examples:
/// - `projects/alpha.md#` (root section, no heading)
/// - `projects/alpha.md#Goals` (under # Goals)
/// - `projects/alpha.md#Goals/Q1` (under ## Q1 under # Goals)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SectionId(String);

impl SectionId {
    /// Create section ID from note path and heading path.
    pub fn new(note_path: &VaultPath, heading_path: &[String]) -> Self {
        let section_path = heading_path.join("/");
        Self(format!("{}#{}", note_path.as_str(), section_path))
    }

    /// Extract the note path portion.
    pub fn note_path(&self) -> VaultPath {
        let (path, _) = self.0.split_once('#').unwrap_or((&self.0, ""));
        VaultPath::new(path).expect("valid path in section id")
    }

    /// Extract the section/heading path portion.
    pub fn section_path(&self) -> Vec<String> {
        let (_, section) = self.0.split_once('#').unwrap_or(("", ""));
        if section.is_empty() {
            Vec::new()
        } else {
            section.split('/').map(String::from).collect()
        }
    }

    /// Get the raw string representation.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SectionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

// ---------------------------------------------------------------------------
// InMemoryIndex errors
// ---------------------------------------------------------------------------

/// Errors specific to in-memory index operations.
#[derive(Debug, Error)]
pub enum InMemoryIndexError {
    #[error("BM25 error: {0}")]
    BM25(#[from] BM25Error),
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
// Persistence format
// ---------------------------------------------------------------------------

const SNAPSHOT_VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct IndexSnapshot {
    version: u32,
    meta: IndexMeta,
    sections: std::collections::HashMap<SectionId, SectionEntry>,
    tag_index: TagIndex,
    bm25_index: BM25Index,
}

// ---------------------------------------------------------------------------
// InMemoryIndexInner
// ---------------------------------------------------------------------------

struct InMemoryIndexInner {
    sections: std::collections::HashMap<SectionId, SectionEntry>,
    tag_index: TagIndex,
    bm25_index: BM25Index,
    meta: IndexMeta,
}

impl InMemoryIndexInner {
    fn new(tokenizer_id: &str) -> Result<Self, InMemoryIndexError> {
        Ok(Self {
            sections: std::collections::HashMap::new(),
            tag_index: TagIndex::new(),
            bm25_index: BM25Index::new(tokenizer_id)?,
            meta: IndexMeta {
                note_count: 0,
                last_indexed: None,
                tokenizer_id: tokenizer_id.to_string(),
            },
        })
    }

    fn from_snapshot(mut snapshot: IndexSnapshot) -> Result<Self, InMemoryIndexError> {
        // Reinitialize tokenizer after deserialization
        snapshot.bm25_index.init_tokenizer()?;

        Ok(Self {
            sections: snapshot.sections,
            tag_index: snapshot.tag_index,
            bm25_index: snapshot.bm25_index,
            meta: snapshot.meta,
        })
    }

    fn to_snapshot(&self) -> IndexSnapshot {
        IndexSnapshot {
            version: SNAPSHOT_VERSION,
            meta: self.meta.clone(),
            sections: self.sections.clone(),
            tag_index: self.tag_index.clone(),
            bm25_index: self.bm25_index.clone_for_snapshot(),
        }
    }
}

// ---------------------------------------------------------------------------
// InMemoryIndex
// ---------------------------------------------------------------------------

/// In-memory index with optional persistence.
///
/// Uses `tokio::sync::RwLock` for thread-safe async access. Supports:
/// - BM25 full-text search via HuggingFace tokenizers
/// - Tag-based filtering with include/exclude
/// - Auto-persistence after mutations
pub struct InMemoryIndex {
    inner: RwLock<InMemoryIndexInner>,
    persistence_path: Option<PathBuf>,
}

impl InMemoryIndex {
    /// Create an ephemeral index (no persistence).
    pub fn new(tokenizer_id: &str) -> Result<Self, InMemoryIndexError> {
        Ok(Self {
            inner: RwLock::new(InMemoryIndexInner::new(tokenizer_id)?),
            persistence_path: None,
        })
    }

    /// Create an index with persistence. Loads from file if exists.
    pub async fn with_persistence(
        path: impl Into<PathBuf>,
        tokenizer_id: &str,
    ) -> Result<Self, InMemoryIndexError> {
        let path = path.into();

        let inner = if path.exists() {
            let data = tokio::fs::read(&path).await?;
            let snapshot: IndexSnapshot = serde_json::from_slice(&data)?;
            InMemoryIndexInner::from_snapshot(snapshot)?
        } else {
            InMemoryIndexInner::new(tokenizer_id)?
        };

        Ok(Self {
            inner: RwLock::new(inner),
            persistence_path: Some(path),
        })
    }

    /// Save the index to disk (atomic write via temp file + rename).
    async fn persist(&self) -> Result<(), InMemoryIndexError> {
        let Some(path) = &self.persistence_path else {
            return Ok(());
        };

        let snapshot = {
            let inner = self.inner.read().await;
            inner.to_snapshot()
        };

        let data = serde_json::to_vec(&snapshot)?;

        // Atomic write: temp file + rename
        let temp_path = path.with_extension("tmp");
        tokio::fs::write(&temp_path, &data).await?;
        tokio::fs::rename(&temp_path, path).await?;

        Ok(())
    }

    /// Index a note, extracting all sections.
    fn index_note(inner: &mut InMemoryIndexInner, note: &Note, note_path: &VaultPath) {
        // Get frontmatter tags (attached to all sections)
        let frontmatter_tags: Vec<String> = note.frontmatter.tags.clone();

        for section in &note.sections {
            let section_id = SectionId::new(note_path, &section.heading_path);

            // Combine frontmatter tags with inline tags
            let mut all_tags: Vec<String> = frontmatter_tags.clone();
            all_tags.extend(section.tags.iter().cloned());

            // Convert links to IndexLink
            let links: Vec<IndexLink> = section.links.iter().map(|link| link.into()).collect();

            let entry = SectionEntry {
                note_path: note_path.clone(),
                heading_path: section.heading_path.clone(),
                tags: all_tags.clone(),
                links,
                word_count: section.word_count,
                revision: crate::common::RevisionToken::from(chrono::Utc::now().to_rfc3339()),
            };

            // Add to BM25 index
            inner
                .bm25_index
                .add_document(section_id.clone(), &section.content);

            // Add to tag index
            inner.tag_index.add(section_id.clone(), &all_tags);

            // Store section entry
            inner.sections.insert(section_id, entry);
        }
    }

    /// Remove all sections for a note path.
    fn remove_note_sections(inner: &mut InMemoryIndexInner, note_path: &VaultPath) {
        // Find all section IDs for this note
        let section_ids: Vec<SectionId> = inner
            .sections
            .keys()
            .filter(|id| &id.note_path() == note_path)
            .cloned()
            .collect();

        for section_id in section_ids {
            inner.sections.remove(&section_id);
            inner.tag_index.remove(&section_id);
            inner.bm25_index.remove_document(&section_id);
        }
    }
}

// Implement Index trait
impl Index for InMemoryIndex {
    async fn get(&self, path: &VaultPath) -> Result<Vec<SectionEntry>, IndexError> {
        let inner = self.inner.read().await;

        let sections: Vec<SectionEntry> = inner
            .sections
            .iter()
            .filter(|(id, _)| &id.note_path() == path)
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
            let mut inner = self.inner.write().await;

            // Remove existing sections for this note
            Self::remove_note_sections(&mut inner, note_path);

            // Index the note
            Self::index_note(&mut inner, note, note_path);

            // Update metadata
            inner.meta.note_count = inner
                .sections
                .values()
                .map(|e| &e.note_path)
                .collect::<HashSet<_>>()
                .len();
            inner.meta.last_indexed = Some(chrono::Utc::now());
        }

        self.persist().await.map_err(Into::into)
    }

    async fn update_bulk(&self, notes: &[Note]) -> Result<(), IndexError> {
        {
            let mut inner = self.inner.write().await;

            for note in notes {
                let Some(note_path) = &note.path else {
                    continue;
                };

                Self::remove_note_sections(&mut inner, note_path);
                Self::index_note(&mut inner, note, note_path);
            }

            // Update metadata
            inner.meta.note_count = inner
                .sections
                .values()
                .map(|e| &e.note_path)
                .collect::<HashSet<_>>()
                .len();
            inner.meta.last_indexed = Some(chrono::Utc::now());
        }

        self.persist().await.map_err(Into::into)
    }

    async fn remove(&self, path: &VaultPath) -> Result<(), IndexError> {
        {
            let mut inner = self.inner.write().await;
            Self::remove_note_sections(&mut inner, path);

            // Update note count
            inner.meta.note_count = inner
                .sections
                .values()
                .map(|e| &e.note_path)
                .collect::<HashSet<_>>()
                .len();
        }

        self.persist().await.map_err(Into::into)
    }

    async fn remove_bulk(&self, paths: &[VaultPath]) -> Result<(), IndexError> {
        {
            let mut inner = self.inner.write().await;

            for path in paths {
                Self::remove_note_sections(&mut inner, path);
            }

            // Update note count
            inner.meta.note_count = inner
                .sections
                .values()
                .map(|e| &e.note_path)
                .collect::<HashSet<_>>()
                .len();
        }

        self.persist().await.map_err(Into::into)
    }

    async fn search(
        &self,
        query: &str,
        params: SearchParams,
    ) -> Result<Vec<(SectionEntry, f32)>, IndexError> {
        let inner = self.inner.read().await;

        // Build tag filters from params
        let include_tags: Option<HashSet<String>> =
            params.tags.as_ref().map(|t| t.iter().cloned().collect());

        // Get BM25 search results
        let bm25_limit = params.limit + params.offset + 1000; // Get extra for filtering
        let bm25_results = inner.bm25_index.search(query, bm25_limit);

        // Filter results
        let mut results: Vec<(SectionEntry, f32)> = bm25_results
            .into_iter()
            .filter_map(|(section_id, score)| {
                let entry = inner.sections.get(&section_id)?;

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

        // Already sorted by BM25 score descending
        results.truncate(params.limit);

        Ok(results)
    }

    async fn list(
        &self,
        folder: Option<&VaultPath>,
        recursive: bool,
    ) -> Result<Vec<SectionEntry>, IndexError> {
        let inner = self.inner.read().await;

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
        let inner = self.inner.read().await;

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
        let inner = self.inner.read().await;

        let links: Vec<IndexLink> = inner
            .sections
            .values()
            .filter(|entry| &entry.note_path == path)
            .flat_map(|entry| entry.links.iter().cloned())
            .collect();

        Ok(links)
    }

    async fn meta(&self) -> Result<IndexMeta, IndexError> {
        let inner = self.inner.read().await;
        Ok(inner.meta.clone())
    }

    async fn set_meta(&self, meta: IndexMeta) -> Result<(), IndexError> {
        {
            let mut inner = self.inner.write().await;
            inner.meta = meta;
        }

        self.persist().await.map_err(Into::into)
    }

    async fn clear(&self) -> Result<(), IndexError> {
        {
            let mut inner = self.inner.write().await;
            inner.sections.clear();
            inner.tag_index.clear();
            inner.bm25_index.clear();
            inner.meta.note_count = 0;
        }

        self.persist().await.map_err(Into::into)
    }

    async fn count(&self) -> Result<usize, IndexError> {
        let inner = self.inner.read().await;
        Ok(inner.sections.len())
    }
}

// ---------------------------------------------------------------------------
// Link conversion
// ---------------------------------------------------------------------------

impl From<&crate::note::Link> for IndexLink {
    fn from(link: &crate::note::Link) -> Self {
        use crate::note::Link;
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

    fn make_note(path: &str, content: &str) -> Note {
        let mut note = Note::from(content);
        note.path = Some(VaultPath::new(path).unwrap());
        note
    }

    #[test]
    fn section_id_parsing() {
        let path = VaultPath::new("projects/alpha.md").unwrap();

        // Root section
        let id = SectionId::new(&path, &[]);
        assert_eq!(id.as_str(), "projects/alpha.md#");
        assert_eq!(id.note_path(), path);
        assert!(id.section_path().is_empty());

        // Single heading
        let id = SectionId::new(&path, &["Goals".to_string()]);
        assert_eq!(id.as_str(), "projects/alpha.md#Goals");
        assert_eq!(id.section_path(), vec!["Goals"]);

        // Nested headings
        let id = SectionId::new(&path, &["Goals".to_string(), "Q1".to_string()]);
        assert_eq!(id.as_str(), "projects/alpha.md#Goals/Q1");
        assert_eq!(id.section_path(), vec!["Goals", "Q1"]);
    }

    #[tokio::test]
    async fn index_and_search() {
        let index = InMemoryIndex::new("bert-base-uncased").unwrap();

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
        let index = InMemoryIndex::new("bert-base-uncased").unwrap();

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
        let index = InMemoryIndex::new("bert-base-uncased").unwrap();

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
        let index = InMemoryIndex::new("bert-base-uncased").unwrap();

        let note = make_note("note.md", "Content.\n");
        index.update(&note).await.unwrap();

        assert_eq!(index.count().await.unwrap(), 1);

        index.clear().await.unwrap();

        assert_eq!(index.count().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn search_with_folder_filter() {
        let index = InMemoryIndex::new("bert-base-uncased").unwrap();

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
        let index = InMemoryIndex::new("bert-base-uncased").unwrap();

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
        let index = InMemoryIndex::new("bert-base-uncased").unwrap();

        let note = make_note("note.md", "See [[target1]] and [[target2|alias]].\n");
        index.update(&note).await.unwrap();

        let path = VaultPath::new("note.md").unwrap();
        let links = index.forward_links(&path).await.unwrap();

        assert_eq!(links.len(), 2);
    }
}
