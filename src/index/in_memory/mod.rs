//! In-memory index implementation with BM25 + tag scoring and RRF fusion.

mod bm25;
pub mod rrf;
pub mod scorer;
mod tags;

pub use bm25::{BM25Config, BM25Index, BM25Snapshot};
pub use rrf::{RRF, RRFConfig};
pub use scorer::Scorer;
pub use tags::{TagIndex, TagIndexConfig, TagIndexError};

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::RwLock;

use crate::common::{Configurable, VaultPath};
use crate::note_handler::Note;

use super::config::InMemoryIndexConfig;
use super::{Index, IndexEntry, IndexError, IndexLink, IndexMeta, SearchParams};

// ---------------------------------------------------------------------------
// InMemoryIndex errors
// ---------------------------------------------------------------------------

/// Errors specific to in-memory index operations.
#[derive(Debug, Error)]
pub enum InMemoryIndexError {
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
// IndexData (persisted state — sections only, scorers rebuild)
// ---------------------------------------------------------------------------

/// Version marker for persistence format. Bump to force rebuild.
const INDEX_DATA_VERSION: u32 = 3;

#[derive(Serialize, Deserialize)]
struct IndexData {
    #[serde(default)]
    version: u32,
    meta: IndexMeta,
    sections: HashMap<VaultPath, IndexEntry>,
    #[serde(default)]
    bm25_snapshot: Option<BM25Snapshot>,
}

impl IndexData {
    fn new() -> Self {
        Self {
            version: INDEX_DATA_VERSION,
            meta: IndexMeta {
                note_count: 0,
                last_indexed: None,
            },
            sections: HashMap::new(),
            bm25_snapshot: None,
        }
    }

    /// Check if a note has any sections in the index.
    fn has_note(&self, note_path: &VaultPath) -> bool {
        self.sections
            .values()
            .any(|e| e.path.note_path().as_ref() == Some(note_path))
    }
}

// ---------------------------------------------------------------------------
// InMemoryIndex
// ---------------------------------------------------------------------------

/// In-memory index with BM25 + tag scoring and RRF fusion.
///
/// Uses `tokio::sync::RwLock` for thread-safe async access. Supports:
/// - BM25 full-text search (StemmingTokenizer, internal)
/// - Tag-based trigram similarity scoring (NgramTokenizer, internal)
/// - RRF fusion of both pipelines
/// - Boolean tag/folder filtering (hard filters applied before scoring)
pub struct InMemoryIndex {
    data: RwLock<IndexData>,
    bm25_index: RwLock<BM25Index>,
    tag_index: RwLock<TagIndex>,
    rrf: RRF,
    bm25_config: BM25Config,
    tag_index_config: TagIndexConfig,
    persistence_path: Option<PathBuf>,
}

impl InMemoryIndex {
    /// Create a new in-memory index.
    ///
    /// If a persistence path is provided and contains valid section data,
    /// sections are loaded from disk. BM25 and tag indexes are always rebuilt
    /// from sections (they own their tokenizers and can't be deserialized).
    pub fn new(
        bm25_index: BM25Index,
        tag_index: TagIndex,
        rrf: RRF,
        persistence_path: Option<PathBuf>,
    ) -> Result<Self, InMemoryIndexError> {
        let mut data = match &persistence_path {
            Some(path) if path.exists() => {
                let bytes = std::fs::read(path)?;
                let loaded: IndexData = serde_json::from_slice(&bytes)?;
                if loaded.version == INDEX_DATA_VERSION {
                    loaded
                } else {
                    tracing::info!("index version changed, rebuilding");
                    IndexData::new()
                }
            }
            _ => IndexData::new(),
        };

        // Rebuild scorer indexes from persisted state
        let mut bm25 = bm25_index;
        let mut tags = tag_index;

        // Restore BM25 from its own snapshot
        if let Some(snapshot) = data.bm25_snapshot.take() {
            bm25.restore(snapshot);
        }

        // Rebuild tag index from persisted section tags
        for (section_path, entry) in &data.sections {
            tags.add(section_path.clone(), &entry.tags);
        }

        let bm25_config = bm25.config();
        let tag_index_config = tags.config();

        Ok(Self {
            data: RwLock::new(data),
            bm25_index: RwLock::new(bm25),
            tag_index: RwLock::new(tags),
            rrf,
            bm25_config,
            tag_index_config,
            persistence_path,
        })
    }

    /// Save the index sections to disk (atomic write via temp file + rename).
    async fn persist(&self) -> Result<(), InMemoryIndexError> {
        let Some(path) = &self.persistence_path else {
            return Ok(());
        };

        let bytes = {
            let mut inner = self.data.write().await;
            let bm25 = self.bm25_index.read().await;
            inner.bm25_snapshot = Some(bm25.snapshot());
            serde_json::to_vec(&*inner)?
        };

        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let temp_path = path.with_extension("tmp");
        tokio::fs::write(&temp_path, &bytes).await?;
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

    /// Index a note, extracting all sections.
    async fn index_note_inner(&self, note: &Note, note_path: &VaultPath) {
        let is_new = {
            let data = self.data.read().await;
            !data.has_note(note_path)
        };

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

            let mut all_tags: Vec<String> = frontmatter_tags.clone();
            all_tags.extend(section.tags.iter().map(|t| t.name().to_string()));

            let links: Vec<IndexLink> = section.links.iter().map(IndexLink::from).collect();

            // Add to BM25 index (tokenizes internally)
            let token_count = {
                let mut bm25 = self.bm25_index.write().await;
                bm25.add_document(section_path.clone(), &section.content)
            };

            // Add to tag index (computes trigrams internally)
            {
                let mut tags = self.tag_index.write().await;
                tags.add(section_path.clone(), &all_tags);
            }

            let entry = IndexEntry {
                path: section_path.clone(),
                tags: all_tags,
                links,
                token_count,
                revision: crate::common::RevisionToken::from(chrono::Utc::now().to_rfc3339()),
            };

            // Store section entry
            {
                let mut data = self.data.write().await;
                data.sections.insert(section_path, entry);
            }
        }

        if is_new {
            let mut data = self.data.write().await;
            data.meta.note_count += 1;
        }
    }

    /// Remove all sections for a note path.
    async fn remove_note_sections_inner(&self, note_path: &VaultPath) {
        let section_paths: Vec<VaultPath> = {
            let data = self.data.read().await;
            data.sections
                .keys()
                .filter(|path| path.note_path().as_ref() == Some(note_path))
                .cloned()
                .collect()
        };

        if !section_paths.is_empty() {
            let mut data = self.data.write().await;
            let mut bm25 = self.bm25_index.write().await;
            let mut tags = self.tag_index.write().await;

            for section_path in section_paths {
                data.sections.remove(&section_path);
                tags.remove(&section_path);
                bm25.remove_document(&section_path);
            }
            data.meta.note_count = data.meta.note_count.saturating_sub(1);
        }
    }
}

impl Configurable for InMemoryIndex {
    type Config = InMemoryIndexConfig;

    fn config(&self) -> Self::Config {
        InMemoryIndexConfig {
            bm25_index: self.bm25_config.clone(),
            tag_index: self.tag_index_config.clone(),
            rrf: self.rrf.config(),
            persistence_path: self.persistence_path.clone(),
        }
    }
}

impl Index for InMemoryIndex {
    async fn get(&self, path: &VaultPath) -> Result<Vec<IndexEntry>, IndexError> {
        let data = self.data.read().await;

        let sections: Vec<IndexEntry> = data
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

        self.remove_note_sections_inner(note_path).await;
        self.index_note_inner(note, note_path).await;

        {
            let mut data = self.data.write().await;
            data.meta.last_indexed = Some(chrono::Utc::now());
        }

        self.persist().await.map_err(Into::into)
    }

    async fn remove(&self, path: &VaultPath) -> Result<(), IndexError> {
        self.remove_note_sections_inner(path).await;
        self.persist().await.map_err(Into::into)
    }

    async fn search(
        &self,
        query: &str,
        params: SearchParams,
    ) -> Result<Vec<(IndexEntry, f32)>, IndexError> {
        if query.is_empty() && params.tags.is_empty() {
            return Ok(Vec::new());
        }

        // Step 1: Hard filters — get candidate section paths
        let candidates = {
            let data = self.data.read().await;
            let tag_index = self.tag_index.read().await;

            let mut candidates: HashSet<VaultPath> = if !params.tags.is_empty() {
                let expanded_tags = tag_index.expand_hierarchical(&params.tags);
                tag_index.filter(Some(&expanded_tags), None)
            } else {
                data.sections.keys().cloned().collect()
            };

            // Apply folder filters
            if !params.folders.is_empty() {
                candidates.retain(|section_path| {
                    if let Some(entry) = data.sections.get(section_path) {
                        params.folders.iter().any(|f| entry.path.is_under_folder(f))
                    } else {
                        false
                    }
                });
            }

            candidates
        };

        if candidates.is_empty() {
            return Ok(Vec::new());
        }

        // Step 2: Score candidates through both pipelines
        let bm25_results = {
            let bm25 = self.bm25_index.read().await;
            bm25.score(query, &candidates)
        };

        let tag_results = {
            let tags = self.tag_index.read().await;
            tags.score(query, &candidates)
        };

        // Step 3: RRF fusion
        let fused = self.rrf.fuse(&[bm25_results, tag_results], params.limit);

        // Step 4: Map to (IndexEntry, f32)
        let data = self.data.read().await;
        let results: Vec<(IndexEntry, f32)> = fused
            .into_iter()
            .filter_map(|(path, score)| {
                let entry = data.sections.get(&path)?;
                Some((entry.clone(), score))
            })
            .collect();

        Ok(results)
    }

    async fn list(
        &self,
        folder: Option<&VaultPath>,
        recursive: bool,
    ) -> Result<Vec<IndexEntry>, IndexError> {
        let data = self.data.read().await;

        let sections: Vec<IndexEntry> = data
            .sections
            .values()
            .filter(|entry| match folder {
                None => true,
                Some(f) if recursive => entry.path.is_under_folder(f),
                Some(f) => entry.path.note_path().is_some_and(|np| np.is_in_folder(f)),
            })
            .cloned()
            .collect();

        Ok(sections)
    }

    async fn backlinks(&self, target: &str) -> Result<Vec<IndexEntry>, IndexError> {
        let data = self.data.read().await;

        let sections: Vec<IndexEntry> = data
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
        let data = self.data.read().await;

        let links: Vec<IndexLink> = data
            .sections
            .values()
            .filter(|entry| entry.path.note_path().as_ref() == Some(path))
            .flat_map(|entry| entry.links.iter().cloned())
            .collect();

        Ok(links)
    }

    async fn meta(&self) -> Result<IndexMeta, IndexError> {
        let data = self.data.read().await;
        Ok(data.meta.clone())
    }

    async fn set_meta(&self, meta: IndexMeta) -> Result<(), IndexError> {
        {
            let mut data = self.data.write().await;
            data.meta = meta;
        }

        self.persist().await.map_err(Into::into)
    }

    async fn clear(&self) -> Result<(), IndexError> {
        {
            let mut data = self.data.write().await;
            data.sections.clear();
            data.meta.note_count = 0;
        }
        {
            let mut bm25 = self.bm25_index.write().await;
            bm25.clear();
        }
        {
            let mut tags = self.tag_index.write().await;
            tags.clear();
        }

        self.persist().await.map_err(Into::into)
    }

    async fn count(&self) -> Result<usize, IndexError> {
        let data = self.data.read().await;
        Ok(data.sections.len())
    }

    async fn update_bulk(&self, notes: &[Note]) -> Result<(), IndexError> {
        for note in notes {
            let Some(note_path) = &note.path else {
                continue;
            };

            self.remove_note_sections_inner(note_path).await;
            self.index_note_inner(note, note_path).await;
        }

        {
            let mut data = self.data.write().await;
            data.meta.last_indexed = Some(chrono::Utc::now());
        }

        self.persist().await.map_err(Into::into)
    }

    async fn remove_bulk(&self, paths: &[VaultPath]) -> Result<(), IndexError> {
        for path in paths {
            self.remove_note_sections_inner(path).await;
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
        InMemoryIndexConfig::default().build().unwrap()
    }

    fn make_note(path: &str, content: &str) -> Note {
        let mut note = Note::from(content);
        note.path = Some(VaultPath::new(path).unwrap());
        note
    }

    #[test]
    fn section_path_construction() {
        let path = VaultPath::new("projects/alpha.md").unwrap();

        let sp = InMemoryIndex::make_section_path(&path, &[]).unwrap();
        assert_eq!(sp.as_str(), "projects/alpha.md#");
        assert_eq!(sp.note_path(), Some(path.clone()));
        assert!(sp.section_headings().is_empty());

        let sp = InMemoryIndex::make_section_path(&path, &["Goals".to_string()]).unwrap();
        assert_eq!(sp.as_str(), "projects/alpha.md#Goals");
        assert_eq!(sp.section_headings(), vec!["Goals"]);

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
    async fn search_with_tag_filter() {
        let index = make_index();

        let note1 = make_note(
            "note1.md",
            "---\ntags:\n  - rust\n  - programming\n---\n# Rust\n\nRust content.\n",
        );
        let note2 = make_note(
            "note2.md",
            "---\ntags:\n  - python\n---\n# Python\n\nPython content.\n",
        );

        index.update(&note1).await.unwrap();
        index.update(&note2).await.unwrap();

        let results = index
            .search(
                "content",
                SearchParams {
                    tags: vec!["rust".to_string()],
                    limit: 10,
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        // All results should be from notes with "rust" tag
        assert!(
            results
                .iter()
                .all(|(e, _)| e.tags.contains(&"rust".to_string()))
        );
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
                    folders: vec![VaultPath::new("projects/").unwrap()],
                    limit: 10,
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        assert!(!results.is_empty());
        assert!(results.iter().all(|(e, _)| {
            e.path
                .note_path()
                .unwrap()
                .as_str()
                .starts_with("projects/")
        }));
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
    async fn backlinks() {
        let index = make_index();

        let note1 = make_note("note1.md", "Check [[note2]] for details.\n");
        let note2 = make_note("note2.md", "Reference content.\n");

        index.update(&note1).await.unwrap();
        index.update(&note2).await.unwrap();

        let backlinks = index.backlinks("note2").await.unwrap();

        assert_eq!(backlinks.len(), 1);
        assert_eq!(backlinks[0].path.note_path().unwrap().as_str(), "note1.md");
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
    async fn empty_query_and_no_tags_returns_empty() {
        let index = make_index();

        let note = make_note("note.md", "# Test\n\nContent.\n");
        index.update(&note).await.unwrap();

        let results = index
            .search(
                "",
                SearchParams {
                    limit: 10,
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn hierarchical_tag_expansion() {
        let index = make_index();

        let note = make_note(
            "note.md",
            "---\ntags:\n  - project/alpha\n---\n# Alpha\n\nProject Alpha content.\n",
        );
        index.update(&note).await.unwrap();

        // Filter by parent tag "project" should match "project/alpha"
        let results = index
            .search(
                "alpha",
                SearchParams {
                    tags: vec!["project".to_string()],
                    limit: 10,
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        assert!(!results.is_empty());
    }
}
