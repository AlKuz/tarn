//! Tag-based inverted index with trigram similarity scoring.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::common::{Buildable, Configurable, VaultPath};
use crate::tokenizer::NgramTokenizer;

use super::scorer::Scorer;

/// Default n-gram size for tag trigrams.
const DEFAULT_N: usize = 3;
const DEFAULT_THRESHOLD: f32 = 0.0;

/// Configuration for the tag index.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TagIndexConfig {
    /// N-gram size for trigram scoring (default: 3).
    #[serde(default = "default_n")]
    pub n: usize,
    /// Minimum Jaccard similarity to include in results (default: 0.0).
    #[serde(default)]
    pub threshold: f32,
}

fn default_n() -> usize {
    DEFAULT_N
}

impl Default for TagIndexConfig {
    fn default() -> Self {
        Self {
            n: DEFAULT_N,
            threshold: DEFAULT_THRESHOLD,
        }
    }
}

impl Buildable for TagIndexConfig {
    type Target = TagIndex;
    type Error = std::convert::Infallible;

    fn build(&self) -> Result<Self::Target, Self::Error> {
        Ok(TagIndex {
            ngram_tokenizer: NgramTokenizer::new(self.n),
            threshold: self.threshold,
            index: HashMap::new(),
            reverse: HashMap::new(),
            section_trigrams: HashMap::new(),
        })
    }
}

/// Inverted index for tag-based filtering and trigram similarity scoring.
///
/// Maintains bidirectional mappings for boolean filtering and precomputed
/// trigrams for Jaccard similarity scoring. Implements `Scorer` for
/// integration with RRF fusion.
pub struct TagIndex {
    /// Internal n-gram tokenizer for trigram computation.
    ngram_tokenizer: NgramTokenizer,
    /// Minimum similarity threshold.
    threshold: f32,
    /// tag -> set of section paths
    index: HashMap<String, HashSet<VaultPath>>,
    /// section_path -> tags (for efficient removal)
    reverse: HashMap<VaultPath, HashSet<String>>,
    /// section_path -> precomputed trigrams from concatenated tags
    section_trigrams: HashMap<VaultPath, HashSet<String>>,
}

impl TagIndex {
    /// Add tags for a section.
    ///
    /// Also precomputes trigrams from the concatenated tag names.
    pub fn add(&mut self, section_path: VaultPath, tags: &[String]) {
        let tag_set: HashSet<String> = tags.iter().cloned().collect();

        for tag in &tag_set {
            self.index
                .entry(tag.clone())
                .or_default()
                .insert(section_path.clone());
        }

        // Precompute trigrams from concatenated tags
        if !tags.is_empty() {
            let concatenated = tags.join(" ");
            let trigrams: HashSet<String> = self
                .ngram_tokenizer
                .tokenize(&concatenated)
                .into_iter()
                .collect();
            self.section_trigrams.insert(section_path.clone(), trigrams);
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
        self.section_trigrams.remove(section_path);
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
        self.section_trigrams.clear();
    }

    /// Get all tags for a section.
    pub fn tags_for_section(&self, section_path: &VaultPath) -> Option<&HashSet<String>> {
        self.reverse.get(section_path)
    }
}

impl Scorer for TagIndex {
    /// Score candidate sections by Jaccard similarity between query trigrams
    /// and section tag trigrams.
    fn score(&self, query: &str, candidates: &HashSet<VaultPath>) -> Vec<(VaultPath, f32)> {
        if query.is_empty() || candidates.is_empty() {
            return Vec::new();
        }

        let query_trigrams: HashSet<String> =
            self.ngram_tokenizer.tokenize(query).into_iter().collect();

        if query_trigrams.is_empty() {
            return Vec::new();
        }

        let mut results: Vec<(VaultPath, f32)> = candidates
            .iter()
            .filter_map(|path| {
                let section_trigrams = self.section_trigrams.get(path)?;
                if section_trigrams.is_empty() {
                    return None;
                }

                let intersection = query_trigrams.intersection(section_trigrams).count();
                if intersection == 0 {
                    return None;
                }

                let union = query_trigrams.union(section_trigrams).count();
                let jaccard = intersection as f32 / union as f32;

                if jaccard > self.threshold {
                    Some((path.clone(), jaccard))
                } else {
                    None
                }
            })
            .collect();

        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results
    }
}

impl Configurable for TagIndex {
    type Config = TagIndexConfig;

    fn config(&self) -> Self::Config {
        TagIndexConfig {
            n: self.ngram_tokenizer.config().n,
            threshold: self.threshold,
        }
    }
}

impl std::fmt::Debug for TagIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TagIndex")
            .field("tag_count", &self.index.len())
            .field("section_count", &self.reverse.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn section_path(path: &str, headings: &[&str]) -> VaultPath {
        let heading_path = headings.join("/");
        VaultPath::new(format!("{path}#{heading_path}")).unwrap()
    }

    fn make_index() -> TagIndex {
        TagIndexConfig::default().build().unwrap()
    }

    // --- Boolean filtering tests (existing) ---

    #[test]
    fn add_and_filter_by_include() {
        let mut index = make_index();

        let s1 = section_path("note1.md", &[]);
        let s2 = section_path("note2.md", &[]);
        let s3 = section_path("note3.md", &[]);

        index.add(s1.clone(), &["rust".into(), "programming".into()]);
        index.add(s2.clone(), &["rust".into(), "web".into()]);
        index.add(s3.clone(), &["python".into()]);

        let rust_tags: HashSet<String> = ["rust".into()].into();
        let result = index.filter(Some(&rust_tags), None);
        assert!(result.contains(&s1));
        assert!(result.contains(&s2));
        assert!(!result.contains(&s3));
    }

    #[test]
    fn filter_by_exclude() {
        let mut index = make_index();

        let s1 = section_path("note1.md", &[]);
        let s2 = section_path("note2.md", &[]);

        index.add(s1.clone(), &["rust".into(), "draft".into()]);
        index.add(s2.clone(), &["rust".into()]);

        let exclude: HashSet<String> = ["draft".into()].into();
        let result = index.filter(None, Some(&exclude));
        assert!(!result.contains(&s1));
        assert!(result.contains(&s2));
    }

    #[test]
    fn filter_include_and_exclude() {
        let mut index = make_index();

        let s1 = section_path("note1.md", &[]);
        let s2 = section_path("note2.md", &[]);
        let s3 = section_path("note3.md", &[]);

        index.add(s1.clone(), &["rust".into(), "published".into()]);
        index.add(s2.clone(), &["rust".into(), "draft".into()]);
        index.add(s3.clone(), &["python".into(), "published".into()]);

        let include: HashSet<String> = ["rust".into()].into();
        let exclude: HashSet<String> = ["draft".into()].into();
        let result = index.filter(Some(&include), Some(&exclude));

        assert!(result.contains(&s1));
        assert!(!result.contains(&s2));
        assert!(!result.contains(&s3));
    }

    #[test]
    fn remove_section() {
        let mut index = make_index();

        let s1 = section_path("note1.md", &[]);
        index.add(s1.clone(), &["rust".into(), "programming".into()]);

        assert!(index.tags_for_section(&s1).is_some());

        index.remove(&s1);

        assert!(index.tags_for_section(&s1).is_none());

        let rust_tags: HashSet<String> = ["rust".into()].into();
        let result = index.filter(Some(&rust_tags), None);
        assert!(result.is_empty());
    }

    #[test]
    fn clear_removes_all() {
        let mut index = make_index();

        let s1 = section_path("note1.md", &[]);
        index.add(s1.clone(), &["rust".into()]);

        index.clear();

        assert!(index.filter(None, None).is_empty());
        assert!(index.section_trigrams.is_empty());
    }

    #[test]
    fn empty_include_returns_all() {
        let mut index = make_index();

        let s1 = section_path("note1.md", &[]);
        let s2 = section_path("note2.md", &[]);

        index.add(s1.clone(), &["rust".into()]);
        index.add(s2.clone(), &["python".into()]);

        let result = index.filter(None, None);
        assert_eq!(result.len(), 2);
    }

    // --- Trigram scoring tests ---

    #[test]
    fn score_exact_tag_match() {
        let mut index = make_index();

        let s1 = section_path("note1.md", &[]);
        index.add(s1.clone(), &["rust".into()]);

        let candidates: HashSet<VaultPath> = [s1.clone()].into();
        let results = index.score("rust", &candidates);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, s1);
        // Exact match should have high Jaccard similarity
        assert!(results[0].1 > 0.5);
    }

    #[test]
    fn score_partial_overlap() {
        let mut index = make_index();

        let s1 = section_path("note1.md", &[]);
        let s2 = section_path("note2.md", &[]);

        index.add(s1.clone(), &["rust".into(), "programming".into()]);
        index.add(s2.clone(), &["python".into(), "scripting".into()]);

        let candidates: HashSet<VaultPath> = [s1.clone(), s2.clone()].into();
        let results = index.score("rust", &candidates);

        // s1 should score higher (has "rust" tag)
        assert!(!results.is_empty());
        assert_eq!(results[0].0, s1);
    }

    #[test]
    fn score_respects_candidates() {
        let mut index = make_index();

        let s1 = section_path("note1.md", &[]);
        let s2 = section_path("note2.md", &[]);

        index.add(s1.clone(), &["rust".into()]);
        index.add(s2.clone(), &["rust".into()]);

        // Only s2 is a candidate
        let candidates: HashSet<VaultPath> = [s2.clone()].into();
        let results = index.score("rust", &candidates);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, s2);
    }

    #[test]
    fn score_empty_query_returns_empty() {
        let mut index = make_index();

        let s1 = section_path("note1.md", &[]);
        index.add(s1.clone(), &["rust".into()]);

        let candidates: HashSet<VaultPath> = [s1].into();
        assert!(index.score("", &candidates).is_empty());
    }

    #[test]
    fn score_empty_candidates_returns_empty() {
        let mut index = make_index();

        let s1 = section_path("note1.md", &[]);
        index.add(s1, &["rust".into()]);

        let empty: HashSet<VaultPath> = HashSet::new();
        assert!(index.score("rust", &empty).is_empty());
    }

    #[test]
    fn score_no_tags_section_excluded() {
        let mut index = make_index();

        let s1 = section_path("note1.md", &[]);
        // Add with empty tags — no trigrams generated
        index.add(s1.clone(), &[]);

        let candidates: HashSet<VaultPath> = [s1].into();
        assert!(index.score("rust", &candidates).is_empty());
    }

    #[test]
    fn threshold_filters_low_scores() {
        let config = TagIndexConfig {
            threshold: 0.99, // Very high threshold
            ..Default::default()
        };
        let mut index = config.build().unwrap();

        let s1 = section_path("note1.md", &[]);
        index.add(s1.clone(), &["rust".into(), "programming".into()]);

        let candidates: HashSet<VaultPath> = [s1].into();
        // "web" has low overlap with "rust programming"
        assert!(index.score("web", &candidates).is_empty());
    }

    #[test]
    fn config_roundtrip() {
        let config = TagIndexConfig {
            n: 4,
            threshold: 0.1,
        };
        let index = config.build().unwrap();
        assert_eq!(index.config(), config);
    }

    #[test]
    fn default_config() {
        let config = TagIndexConfig::default();
        assert_eq!(config.n, 3);
        assert!((config.threshold - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn remove_cleans_trigrams() {
        let mut index = make_index();

        let s1 = section_path("note1.md", &[]);
        index.add(s1.clone(), &["rust".into()]);
        assert!(index.section_trigrams.contains_key(&s1));

        index.remove(&s1);
        assert!(!index.section_trigrams.contains_key(&s1));
    }
}
