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
use super::{Index, IndexEntry, IndexError, IndexLink, IndexMeta, NoteResult};

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
// IndexData (persistence format only)
// ---------------------------------------------------------------------------

/// Version marker for persistence format. Bump to force rebuild.
const INDEX_DATA_VERSION: u32 = 3;

/// Persistence format for the index.
///
/// Only used for serialization/deserialization — not as runtime state.
#[derive(Serialize, Deserialize)]
struct IndexData {
    #[serde(default)]
    version: u32,
    #[serde(default)]
    meta: IndexMeta,
    #[serde(default)]
    sections: HashMap<VaultPath, IndexEntry>,
    #[serde(default)]
    bm25_snapshot: Option<BM25Snapshot>,
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
    sections: RwLock<HashMap<VaultPath, IndexEntry>>,
    meta: RwLock<IndexMeta>,
    bm25_index: RwLock<BM25Index>,
    tag_index: RwLock<TagIndex>,
    rrf: RRF,
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
        let mut loaded_data = match &persistence_path {
            Some(path) if path.exists() => {
                let bytes = std::fs::read(path)?;
                let loaded: IndexData = serde_json::from_slice(&bytes)?;
                if loaded.version == INDEX_DATA_VERSION {
                    Some(loaded)
                } else {
                    tracing::info!("index version changed, rebuilding");
                    None
                }
            }
            _ => None,
        };

        // Rebuild scorer indexes from persisted state
        let mut bm25 = bm25_index;
        let mut tags = tag_index;

        let (sections, meta) = if let Some(ref mut data) = loaded_data {
            // Restore BM25 from its own snapshot
            if let Some(snapshot) = data.bm25_snapshot.take() {
                bm25.restore(snapshot);
            }

            // Rebuild tag index from persisted section tags
            for (section_path, entry) in &data.sections {
                tags.add(section_path.clone(), &entry.tags);
            }

            (
                std::mem::take(&mut data.sections),
                std::mem::take(&mut data.meta),
            )
        } else {
            (
                HashMap::new(),
                IndexMeta {
                    note_count: 0,
                    last_indexed: None,
                },
            )
        };

        Ok(Self {
            sections: RwLock::new(sections),
            meta: RwLock::new(meta),
            bm25_index: RwLock::new(bm25),
            tag_index: RwLock::new(tags),
            rrf,
            persistence_path,
        })
    }

    /// Save the index sections to disk (atomic write via temp file + rename).
    async fn persist(&self) -> Result<(), InMemoryIndexError> {
        let Some(path) = &self.persistence_path else {
            return Ok(());
        };

        let bm25_snapshot = {
            let bm25 = self.bm25_index.read().await;
            bm25.snapshot()
        };

        let bytes = {
            let sections = self.sections.read().await;
            let meta = self.meta.read().await;
            let data = IndexData {
                version: INDEX_DATA_VERSION,
                meta: meta.clone(),
                sections: sections.clone(),
                bm25_snapshot: Some(bm25_snapshot),
            };
            serde_json::to_vec(&data)?
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
            let sections = self.sections.read().await;
            !sections
                .values()
                .any(|e| e.path.note_path().as_ref() == Some(note_path))
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
                let mut sections = self.sections.write().await;
                sections.insert(section_path, entry);
            }
        }

        if is_new {
            let mut meta = self.meta.write().await;
            meta.note_count += 1;
        }
    }

    /// Apply token limit to search results.
    fn apply_token_limit(
        mut results: Vec<(IndexEntry, f32)>,
        limit: usize,
        token_limit: Option<usize>,
    ) -> Vec<(IndexEntry, f32)> {
        results.truncate(limit);

        let Some(max_tokens) = token_limit else {
            return results;
        };

        let mut total_tokens = 0;
        let mut cutoff_idx = results.len();
        for (i, (entry, _)) in results.iter().enumerate() {
            if total_tokens + entry.token_count > max_tokens {
                cutoff_idx = i;
                break;
            }
            total_tokens += entry.token_count;
        }
        results.truncate(cutoff_idx);
        results
    }

    /// Remove all sections for a note path.
    async fn remove_note_sections_inner(&self, note_path: &VaultPath) {
        let section_paths: Vec<VaultPath> = {
            let sections = self.sections.read().await;
            sections
                .keys()
                .filter(|path| path.note_path().as_ref() == Some(note_path))
                .cloned()
                .collect()
        };

        if !section_paths.is_empty() {
            let mut sections = self.sections.write().await;
            let mut bm25 = self.bm25_index.write().await;
            let mut tags = self.tag_index.write().await;

            for section_path in section_paths {
                sections.remove(&section_path);
                tags.remove(&section_path);
                bm25.remove_document(&section_path);
            }
            drop(sections);
            drop(bm25);
            drop(tags);

            let mut meta = self.meta.write().await;
            meta.note_count = meta.note_count.saturating_sub(1);
        }
    }
}

impl Configurable for InMemoryIndex {
    type Config = InMemoryIndexConfig;

    fn config(&self) -> Self::Config {
        InMemoryIndexConfig {
            bm25_index: self.bm25_index.blocking_read().config(),
            tag_index: self.tag_index.blocking_read().config(),
            rrf: self.rrf.config(),
            persistence_path: self.persistence_path.clone(),
        }
    }
}

impl Index for InMemoryIndex {
    async fn get(&self, path: &VaultPath) -> Result<Vec<IndexEntry>, IndexError> {
        let sections_guard = self.sections.read().await;

        let sections: Vec<IndexEntry> = sections_guard
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
            let mut meta = self.meta.write().await;
            meta.last_indexed = Some(chrono::Utc::now());
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
        folders: &[VaultPath],
        tags: &[String],
        limit: usize,
        token_limit: Option<usize>,
        score_threshold: f32,
    ) -> Result<Vec<NoteResult>, IndexError> {
        if query.is_empty() && tags.is_empty() && folders.is_empty() {
            return Ok(Vec::new());
        }

        // Step 1: Hard filters — get candidate section paths
        let candidates = {
            let sections = self.sections.read().await;
            let tag_index = self.tag_index.read().await;

            let mut candidates: HashSet<VaultPath> = if !tags.is_empty() {
                let expanded_tags = tag_index.expand_hierarchical(tags);
                tag_index.filter(Some(&expanded_tags), None)
            } else {
                sections.keys().cloned().collect()
            };

            // Apply folder filters
            if !folders.is_empty() {
                candidates.retain(|section_path| {
                    if let Some(entry) = sections.get(section_path) {
                        folders.iter().any(|f| entry.path.is_under_folder(f))
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

        // When the query text is empty (tag-only or folder-only filter), skip
        // scoring and return all filtered candidates with a uniform score so
        // that callers receive meaningful results.
        if query.is_empty() {
            let sections = self.sections.read().await;
            let mut results: Vec<(IndexEntry, f32)> = candidates
                .into_iter()
                .filter_map(|path| {
                    let entry = sections.get(&path)?;
                    Some((entry.clone(), 1.0))
                })
                .collect();
            results.sort_by(|a, b| a.0.path.cmp(&b.0.path));
            results = Self::apply_token_limit(results, limit, token_limit);
            let scored: Vec<_> = results.into_iter().map(|(e, s)| (e, Some(s))).collect();
            let mut notes = NoteResult::from_entries(&scored);
            notes.sort_by(|a, b| a.path.cmp(&b.path));
            return Ok(notes);
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
        let fused = self.rrf.fuse(&[bm25_results, tag_results], limit);

        // Step 4: Map to (IndexEntry, f32), apply token limit, group by note
        let sections = self.sections.read().await;
        let results: Vec<(IndexEntry, f32)> = fused
            .into_iter()
            .filter_map(|(path, score)| {
                let entry = sections.get(&path)?;
                Some((entry.clone(), score))
            })
            .collect();

        let limited = Self::apply_token_limit(results, limit, token_limit);

        // Apply score threshold before grouping
        let filtered: Vec<_> = limited
            .into_iter()
            .filter(|(_, score)| *score >= score_threshold)
            .collect();

        let scored: Vec<_> = filtered.into_iter().map(|(e, s)| (e, Some(s))).collect();
        let mut notes = NoteResult::from_entries(&scored);
        notes.sort_by(|a, b| {
            b.max_score()
                .partial_cmp(&a.max_score())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        Ok(notes)
    }

    async fn list(
        &self,
        folder: Option<&VaultPath>,
        recursive: bool,
    ) -> Result<Vec<NoteResult>, IndexError> {
        let sections = self.sections.read().await;

        let entries: Vec<(IndexEntry, Option<f32>)> = sections
            .values()
            .filter(|entry| match folder {
                None => true,
                Some(f) if recursive => entry.path.is_under_folder(f),
                Some(f) => entry.path.note_path().is_some_and(|np| np.is_in_folder(f)),
            })
            .cloned()
            .map(|e| (e, None))
            .collect();

        let mut notes = NoteResult::from_entries(&entries);
        notes.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(notes)
    }

    async fn backlinks(&self, target: &str) -> Result<Vec<NoteResult>, IndexError> {
        let sections = self.sections.read().await;

        let entries: Vec<(IndexEntry, Option<f32>)> = sections
            .values()
            .filter(|entry| {
                entry.links.iter().any(|link| match link {
                    IndexLink::Wiki { target: t, .. } => t == target,
                    _ => false,
                })
            })
            .cloned()
            .map(|e| (e, None))
            .collect();

        let mut notes = NoteResult::from_entries(&entries);
        notes.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(notes)
    }

    async fn forward_links(&self, path: &VaultPath) -> Result<Vec<IndexLink>, IndexError> {
        let sections = self.sections.read().await;

        let links: Vec<IndexLink> = sections
            .values()
            .filter(|entry| entry.path.note_path().as_ref() == Some(path))
            .flat_map(|entry| entry.links.iter().cloned())
            .collect();

        Ok(links)
    }

    async fn meta(&self) -> Result<IndexMeta, IndexError> {
        let meta = self.meta.read().await;
        Ok(meta.clone())
    }

    async fn set_meta(&self, meta: IndexMeta) -> Result<(), IndexError> {
        {
            let mut meta_guard = self.meta.write().await;
            *meta_guard = meta;
        }

        self.persist().await.map_err(Into::into)
    }

    async fn clear(&self) -> Result<(), IndexError> {
        {
            let mut sections = self.sections.write().await;
            sections.clear();
        }
        {
            let mut meta = self.meta.write().await;
            meta.note_count = 0;
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
        let sections = self.sections.read().await;
        Ok(sections.len())
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
            let mut meta = self.meta.write().await;
            meta.last_indexed = Some(chrono::Utc::now());
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
            .search("programming language", &[], &[], 10, None, 0.0)
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
            .search("content", &[], &["rust".to_string()], 10, None, 0.0)
            .await
            .unwrap();

        // All results should be from notes with "rust" tag
        assert!(
            results
                .iter()
                .all(|r| r.tags().contains(&"rust".to_string()))
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
                &[VaultPath::new("projects/").unwrap()],
                &[],
                10,
                None,
                0.0,
            )
            .await
            .unwrap();

        assert!(!results.is_empty());
        assert!(
            results
                .iter()
                .all(|r| r.path.as_str().starts_with("projects/"))
        );
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
        assert_eq!(backlinks[0].path.as_str(), "note1.md");
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

        let results = index.search("", &[], &[], 10, None, 0.0).await.unwrap();

        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn persistence_roundtrip() {
        let dir = tempfile::TempDir::new().unwrap();
        let persistence_path = dir.path().join("index.json");

        let config = InMemoryIndexConfig {
            persistence_path: Some(persistence_path.clone()),
            ..Default::default()
        };

        // Create index, add notes, persist via update
        {
            let index = config.build().unwrap();
            let note = make_note("note.md", "# Hello\n\nRust programming content.\n");
            index.update(&note).await.unwrap();
            assert_eq!(index.count().await.unwrap(), 1);
        }

        // Create a new index from the same path — data should load
        {
            let index = config.build().unwrap();
            assert_eq!(index.count().await.unwrap(), 1);

            let path = VaultPath::new("note.md").unwrap();
            let sections = index.get(&path).await.unwrap();
            assert!(!sections.is_empty());

            // BM25 should work after restore
            let results = index.search("rust", &[], &[], 10, None, 0.0).await.unwrap();
            assert!(!results.is_empty());
        }
    }

    #[test]
    fn apply_token_limit_truncates() {
        let entry = |tokens: usize| IndexEntry {
            path: VaultPath::new(format!("note{tokens}.md")).unwrap(),
            tags: vec![],
            links: vec![],
            token_count: tokens,
            revision: crate::common::RevisionToken::from("rev"),
        };

        let results = vec![(entry(100), 0.9), (entry(100), 0.8), (entry(100), 0.7)];

        // Token limit of 150: first entry (100) fits, second would push total to 200 > 150 so it is excluded
        let limited = InMemoryIndex::apply_token_limit(results, 10, Some(150));
        assert_eq!(limited.len(), 1);
    }

    #[tokio::test]
    async fn tag_only_search() {
        let index = make_index();

        let note1 = make_note(
            "tagged.md",
            "---\ntags:\n  - rust\n---\n# Tagged\n\nRust content.\n",
        );
        let note2 = make_note("untagged.md", "# Untagged\n\nOther content.\n");

        index.update(&note1).await.unwrap();
        index.update(&note2).await.unwrap();

        // Empty query + tag filter
        let results = index
            .search("", &[], &["rust".to_string()], 10, None, 0.0)
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].path.as_str(), "tagged.md");
        // Scores should be uniform (1.0) for filter-only
        assert!(results[0].sections.iter().all(|s| s.score.is_some()));
    }

    #[tokio::test]
    async fn meta_and_set_meta() {
        let index = make_index();

        let meta = index.meta().await.unwrap();
        assert_eq!(meta.note_count, 0);

        let new_meta = IndexMeta {
            note_count: 42,
            last_indexed: Some(chrono::Utc::now()),
        };
        index.set_meta(new_meta.clone()).await.unwrap();

        let loaded = index.meta().await.unwrap();
        assert_eq!(loaded.note_count, 42);
    }

    #[tokio::test]
    async fn remove_bulk() {
        let index = make_index();

        let note1 = make_note("a.md", "# A\n\nContent A.\n");
        let note2 = make_note("b.md", "# B\n\nContent B.\n");
        let note3 = make_note("c.md", "# C\n\nContent C.\n");

        index.update(&note1).await.unwrap();
        index.update(&note2).await.unwrap();
        index.update(&note3).await.unwrap();
        assert_eq!(index.count().await.unwrap(), 3);

        let paths = vec![
            VaultPath::new("a.md").unwrap(),
            VaultPath::new("b.md").unwrap(),
        ];
        index.remove_bulk(&paths).await.unwrap();

        assert_eq!(index.count().await.unwrap(), 1);
    }

    #[test]
    fn config_returns_current_state() {
        let index = make_index();
        let config = index.config();
        assert!(config.persistence_path.is_none());
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
            .search("alpha", &[], &["project".to_string()], 10, None, 0.0)
            .await
            .unwrap();

        assert!(!results.is_empty());
    }
}
