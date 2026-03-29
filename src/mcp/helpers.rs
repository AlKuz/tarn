use std::collections::{HashMap, HashSet};

use crate::common::VaultPath;
use crate::index::SectionEntry;

/// Extract a snippet with context around the first match of `query` in `content`.
pub fn extract_snippet(content: &str, query: &str, context_chars: usize) -> String {
    let lower_content = content.to_lowercase();
    let lower_query = query.to_lowercase();

    if let Some(pos) = lower_content.find(&lower_query) {
        let start = content[..pos]
            .rfind(char::is_whitespace)
            .map(|i| i + 1)
            .unwrap_or(pos.saturating_sub(context_chars));
        let end_pos = pos + query.len();
        let end = content[end_pos..]
            .find(char::is_whitespace)
            .map(|i| end_pos + i)
            .unwrap_or((end_pos + context_chars).min(content.len()));

        let prefix = if start > 0 { "..." } else { "" };
        let suffix = if end < content.len() { "..." } else { "" };
        format!("{prefix}{}{suffix}", &content[start..end])
    } else {
        content.chars().take(100).collect::<String>()
    }
}

/// Find direct children of a parent tag in a tag hierarchy.
pub fn find_direct_children(parent: &str, all_tags: &[String]) -> Vec<String> {
    all_tags
        .iter()
        .filter(|other| {
            other.starts_with(parent)
                && other.len() > parent.len()
                && other.as_bytes().get(parent.len()) == Some(&b'/')
                && !other[parent.len() + 1..].contains('/')
        })
        .cloned()
        .collect()
}

/// Helper for aggregating section data into note-level data.
#[derive(Default)]
pub struct NoteAggregate {
    pub title: Option<String>,
    pub tags: HashSet<String>,
    pub token_count: usize,
}

/// Aggregate sections into notes for list operations.
pub fn aggregate_sections_to_notes(sections: &[SectionEntry]) -> HashMap<VaultPath, NoteAggregate> {
    let mut aggregates: HashMap<VaultPath, NoteAggregate> = HashMap::new();

    for section in sections {
        let entry = aggregates.entry(section.note_path.clone()).or_default();

        // Title comes from first heading (root section or first H1)
        if entry.title.is_none() && !section.heading_path.is_empty() {
            entry.title = Some(section.heading_path[0].clone());
        }

        entry.tags.extend(section.tags.iter().cloned());
        entry.token_count += section.token_count;
    }

    aggregates
}
