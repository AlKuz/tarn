//! Index types for entries, results, and metadata.

use std::collections::{BTreeSet, HashMap};

use serde::{Deserialize, Serialize};

use crate::common::VaultPath;

// ---------------------------------------------------------------------------
// Index link types
// ---------------------------------------------------------------------------

/// Simplified link representation for index storage.
///
/// Compared to the full `Link` enum, this drops details like heading references,
/// block refs, embed flags, and titles - keeping only what's needed for queries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IndexLink {
    /// Wiki link: `[[target]]` or `[[target|alias]]`
    Wiki {
        target: String,
        alias: Option<String>,
    },
    /// Markdown link: `[text](url)`
    Markdown { url: String, text: String },
    /// URL autolink: `<https://example.com>`
    Url { url: String },
    /// Email autolink: `<user@example.com>`
    Email { address: String },
}

// ---------------------------------------------------------------------------
// Section entry
// ---------------------------------------------------------------------------

/// An indexed entry keyed by `VaultPath`.
///
/// Currently every entry corresponds to a note section, but the struct is
/// intentionally generic so the index can store other content types in the
/// future.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexEntry {
    /// Full path to this indexed entry (e.g., `note.md#Goals/Q1`).
    pub path: VaultPath,
    /// Tags attached to this entry.
    pub tags: Vec<String>,
    /// Links found in this entry.
    pub links: Vec<IndexLink>,
    /// Token count of the entry content.
    pub token_count: usize,
}

// ---------------------------------------------------------------------------
// Section result
// ---------------------------------------------------------------------------

/// A section within a note-level search/list result.
///
/// Derived from `IndexEntry` but replaces the full `VaultPath` with the
/// extracted heading path (the note-level path lives on the parent `NoteResult`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectionResult {
    /// Heading path within the note (e.g., `["Goals", "Q1"]`).
    pub heading_path: Vec<String>,
    /// Tags on this section (includes frontmatter tags propagated during indexing).
    pub tags: Vec<String>,
    /// Links found in this section.
    pub links: Vec<IndexLink>,
    /// Token count of the section content.
    pub token_count: usize,
    /// Relevance score (present for search, absent for list/backlinks).
    pub score: Option<f32>,
}

// ---------------------------------------------------------------------------
// Note result
// ---------------------------------------------------------------------------

/// A note-level result grouping all matched sections.
///
/// Returned by `Index::search()`, `Index::list()`, and `Index::backlinks()`.
/// Per-note aggregates (tags, links, token count, max score) are computed from
/// sections on demand via methods — no duplicated storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteResult {
    /// Note-level path (e.g., `projects/alpha.md`).
    pub path: VaultPath,
    /// Individual section results.
    pub sections: Vec<SectionResult>,
}

impl NoteResult {
    /// Union of all section tags, deduplicated and sorted.
    pub fn tags(&self) -> Vec<String> {
        self.sections
            .iter()
            .flat_map(|s| s.tags.iter().cloned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    /// Union of all section links.
    pub fn links(&self) -> Vec<IndexLink> {
        self.sections
            .iter()
            .flat_map(|s| s.links.iter().cloned())
            .collect()
    }

    /// Sum of token counts across all sections.
    pub fn total_token_count(&self) -> usize {
        self.sections.iter().map(|s| s.token_count).sum()
    }

    /// Maximum score across sections (None if no sections have scores).
    pub fn max_score(&self) -> Option<f32> {
        self.sections
            .iter()
            .filter_map(|s| s.score)
            .reduce(f32::max)
    }

    /// Group flat index entries into note-level results.
    ///
    /// Sections sharing the same `note_path()` are collected under one `NoteResult`.
    /// Preserves insertion order (first-seen note).
    pub fn from_entries(entries: &[(IndexEntry, Option<f32>)]) -> Vec<NoteResult> {
        let mut note_map: HashMap<VaultPath, NoteResult> = HashMap::new();
        let mut order: Vec<VaultPath> = Vec::new();

        for (entry, score) in entries {
            let note_path = entry.path.note_path().unwrap_or_else(|| entry.path.clone());

            let heading_path = entry.path.section_headings();

            let section = SectionResult {
                heading_path,
                tags: entry.tags.clone(),
                links: entry.links.clone(),
                token_count: entry.token_count,
                score: *score,
            };

            match note_map.get_mut(&note_path) {
                Some(note_result) => {
                    note_result.sections.push(section);
                }
                None => {
                    order.push(note_path.clone());
                    note_map.insert(
                        note_path.clone(),
                        NoteResult {
                            path: note_path,
                            sections: vec![section],
                        },
                    );
                }
            }
        }

        order
            .into_iter()
            .filter_map(|path| note_map.remove(&path))
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Index metadata
// ---------------------------------------------------------------------------

/// Metadata about the index state.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct IndexMeta {
    /// Total number of indexed notes.
    pub note_count: usize,
    /// Timestamp of last indexing operation.
    pub last_indexed: Option<chrono::DateTime<chrono::Utc>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_section(
        tags: Vec<&str>,
        links: Vec<IndexLink>,
        token_count: usize,
        score: Option<f32>,
    ) -> SectionResult {
        SectionResult {
            heading_path: vec![],
            tags: tags.into_iter().map(String::from).collect(),
            links,
            token_count,
            score,
        }
    }

    #[test]
    fn links_unions_all_sections() {
        let note = NoteResult {
            path: VaultPath::new("test.md").unwrap(),
            sections: vec![
                make_section(
                    vec![],
                    vec![IndexLink::Wiki {
                        target: "a".into(),
                        alias: None,
                    }],
                    10,
                    None,
                ),
                make_section(
                    vec![],
                    vec![IndexLink::Url {
                        url: "https://example.com".into(),
                    }],
                    20,
                    None,
                ),
            ],
        };
        assert_eq!(note.links().len(), 2);
    }

    #[test]
    fn total_token_count_sums_sections() {
        let note = NoteResult {
            path: VaultPath::new("test.md").unwrap(),
            sections: vec![
                make_section(vec![], vec![], 100, None),
                make_section(vec![], vec![], 50, None),
            ],
        };
        assert_eq!(note.total_token_count(), 150);
    }

    #[test]
    fn links_empty_sections() {
        let note = NoteResult {
            path: VaultPath::new("test.md").unwrap(),
            sections: vec![],
        };
        assert!(note.links().is_empty());
    }

    #[test]
    fn total_token_count_empty_sections() {
        let note = NoteResult {
            path: VaultPath::new("test.md").unwrap(),
            sections: vec![],
        };
        assert_eq!(note.total_token_count(), 0);
    }
}
