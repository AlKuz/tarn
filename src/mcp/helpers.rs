use std::collections::{HashMap, HashSet};

use crate::common::VaultPath;
use crate::index::IndexEntry;

/// Parse an optional folder string into a validated `VaultPath`.
pub fn parse_folder(folder: Option<&str>) -> Result<Option<VaultPath>, rmcp::ErrorData> {
    folder
        .map(|f| {
            let normalized = format!("{}/", f.trim_end_matches('/'));
            VaultPath::new(normalized)
                .map_err(|e| rmcp::ErrorData::invalid_params(e.to_string(), None))
        })
        .transpose()
}

/// Helper for aggregating section data into note-level data.
#[derive(Default)]
pub struct NoteAggregate {
    pub title: Option<String>,
    pub tags: HashSet<String>,
    pub token_count: usize,
}

/// Aggregate sections into notes for list operations.
pub fn aggregate_sections_to_notes(sections: &[IndexEntry]) -> HashMap<VaultPath, NoteAggregate> {
    let mut aggregates: HashMap<VaultPath, NoteAggregate> = HashMap::new();

    for section in sections {
        let note_path = section
            .path
            .note_path()
            .unwrap_or_else(|| section.path.clone());
        let entry = aggregates.entry(note_path).or_default();

        // Title comes from first heading (root section or first H1)
        let headings = section.path.section_headings();
        if entry.title.is_none() && !headings.is_empty() {
            entry.title = Some(headings[0].clone());
        }

        entry.tags.extend(section.tags.iter().cloned());
        entry.token_count += section.token_count;
    }

    aggregates
}
