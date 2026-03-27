//! Tag-based inverted index for fast tag queries.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::common::VaultPath;

/// Inverted index for tag-based filtering.
///
/// Maintains bidirectional mappings:
/// - tag -> sections (for filtering by tag)
/// - section -> tags (for efficient removal)
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct TagIndex {
    /// tag -> set of section paths
    index: HashMap<String, HashSet<VaultPath>>,
    /// section_path -> tags (for efficient removal)
    reverse: HashMap<VaultPath, HashSet<String>>,
}

impl TagIndex {
    /// Create a new empty tag index.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add tags for a section.
    pub fn add(&mut self, section_path: VaultPath, tags: &[String]) {
        let tag_set: HashSet<String> = tags.iter().cloned().collect();

        for tag in &tag_set {
            self.index
                .entry(tag.clone())
                .or_default()
                .insert(section_path.clone());
        }

        self.reverse.insert(section_path, tag_set);
    }

    /// Remove a section and all its tag associations.
    pub fn remove(&mut self, section_path: &VaultPath) {
        if let Some(tags) = self.reverse.remove(section_path) {
            for tag in tags {
                if let Some(sections) = self.index.get_mut(&tag) {
                    sections.remove(section_path);
                    if sections.is_empty() {
                        self.index.remove(&tag);
                    }
                }
            }
        }
    }

    /// Filter sections by tag criteria.
    ///
    /// - `include`: section must have at least one of these tags (None = no filter)
    /// - `exclude`: section must have none of these tags (None = no filter)
    ///
    /// Returns all matching section paths.
    pub fn filter(
        &self,
        include: Option<&HashSet<String>>,
        exclude: Option<&HashSet<String>>,
    ) -> HashSet<VaultPath> {
        // Start with sections matching include criteria
        let candidates: HashSet<VaultPath> = match include {
            Some(tags) if !tags.is_empty() => {
                // Union of all sections that have any of the include tags
                tags.iter()
                    .filter_map(|tag| self.index.get(tag))
                    .flat_map(|sections| sections.iter().cloned())
                    .collect()
            }
            _ => {
                // No include filter: start with all sections
                self.reverse.keys().cloned().collect()
            }
        };

        // Filter out sections with excluded tags
        match exclude {
            Some(tags) if !tags.is_empty() => candidates
                .into_iter()
                .filter(|section_path| {
                    self.reverse
                        .get(section_path)
                        .is_none_or(|section_tags| section_tags.is_disjoint(tags))
                })
                .collect(),
            _ => candidates,
        }
    }

    /// Clear all tag data.
    pub fn clear(&mut self) {
        self.index.clear();
        self.reverse.clear();
    }

    /// Get all tags for a section.
    pub fn tags_for_section(&self, section_path: &VaultPath) -> Option<&HashSet<String>> {
        self.reverse.get(section_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn section_path(path: &str, headings: &[&str]) -> VaultPath {
        let heading_path = headings.join("/");
        VaultPath::new(format!("{path}#{heading_path}")).unwrap()
    }

    #[test]
    fn add_and_filter_by_include() {
        let mut index = TagIndex::new();

        let s1 = section_path("note1.md", &[]);
        let s2 = section_path("note2.md", &[]);
        let s3 = section_path("note3.md", &[]);

        index.add(s1.clone(), &["rust".into(), "programming".into()]);
        index.add(s2.clone(), &["rust".into(), "web".into()]);
        index.add(s3.clone(), &["python".into()]);

        // Filter by rust tag
        let rust_tags: HashSet<String> = ["rust".into()].into();
        let result = index.filter(Some(&rust_tags), None);
        assert!(result.contains(&s1));
        assert!(result.contains(&s2));
        assert!(!result.contains(&s3));
    }

    #[test]
    fn filter_by_exclude() {
        let mut index = TagIndex::new();

        let s1 = section_path("note1.md", &[]);
        let s2 = section_path("note2.md", &[]);

        index.add(s1.clone(), &["rust".into(), "draft".into()]);
        index.add(s2.clone(), &["rust".into()]);

        // Exclude drafts
        let exclude: HashSet<String> = ["draft".into()].into();
        let result = index.filter(None, Some(&exclude));
        assert!(!result.contains(&s1));
        assert!(result.contains(&s2));
    }

    #[test]
    fn filter_include_and_exclude() {
        let mut index = TagIndex::new();

        let s1 = section_path("note1.md", &[]);
        let s2 = section_path("note2.md", &[]);
        let s3 = section_path("note3.md", &[]);

        index.add(s1.clone(), &["rust".into(), "published".into()]);
        index.add(s2.clone(), &["rust".into(), "draft".into()]);
        index.add(s3.clone(), &["python".into(), "published".into()]);

        // Include rust, exclude draft
        let include: HashSet<String> = ["rust".into()].into();
        let exclude: HashSet<String> = ["draft".into()].into();
        let result = index.filter(Some(&include), Some(&exclude));

        assert!(result.contains(&s1));
        assert!(!result.contains(&s2)); // excluded by draft
        assert!(!result.contains(&s3)); // not included (no rust)
    }

    #[test]
    fn remove_section() {
        let mut index = TagIndex::new();

        let s1 = section_path("note1.md", &[]);
        index.add(s1.clone(), &["rust".into(), "programming".into()]);

        assert!(index.tags_for_section(&s1).is_some());

        index.remove(&s1);

        assert!(index.tags_for_section(&s1).is_none());

        // Rust tag should be cleaned up
        let rust_tags: HashSet<String> = ["rust".into()].into();
        let result = index.filter(Some(&rust_tags), None);
        assert!(result.is_empty());
    }

    #[test]
    fn clear_removes_all() {
        let mut index = TagIndex::new();

        let s1 = section_path("note1.md", &[]);
        index.add(s1.clone(), &["rust".into()]);

        index.clear();

        assert!(index.filter(None, None).is_empty());
    }

    #[test]
    fn empty_include_returns_all() {
        let mut index = TagIndex::new();

        let s1 = section_path("note1.md", &[]);
        let s2 = section_path("note2.md", &[]);

        index.add(s1.clone(), &["rust".into()]);
        index.add(s2.clone(), &["python".into()]);

        let result = index.filter(None, None);
        assert_eq!(result.len(), 2);
    }
}
