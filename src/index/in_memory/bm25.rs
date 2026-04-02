//! BM25 full-text search index with internal stemming tokenizer.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::common::{Buildable, Configurable, VaultPath};
use crate::tokenizer::StemmingTokenizer;
use crate::tokenizer::Tokenizer;

use super::scorer::Scorer;

/// Default BM25 algorithm parameters.
const DEFAULT_K1: f32 = 1.2;
const DEFAULT_B: f32 = 0.75;
const DEFAULT_THRESHOLD: f32 = 0.0;

/// Configuration for BM25 index.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BM25Config {
    /// Term frequency saturation (default: 1.2).
    #[serde(default = "default_k1")]
    pub k1: f32,
    /// Document length normalization (default: 0.75).
    #[serde(default = "default_b")]
    pub b: f32,
    /// Minimum BM25 score to include in results (default: 0.0).
    #[serde(default)]
    pub threshold: f32,
}

fn default_k1() -> f32 {
    DEFAULT_K1
}
fn default_b() -> f32 {
    DEFAULT_B
}

impl Default for BM25Config {
    fn default() -> Self {
        Self {
            k1: DEFAULT_K1,
            b: DEFAULT_B,
            threshold: DEFAULT_THRESHOLD,
        }
    }
}

impl Buildable for BM25Config {
    type Target = BM25Index;
    type Error = std::convert::Infallible;

    fn build(&self) -> Result<Self::Target, Self::Error> {
        Ok(BM25Index {
            tokenizer: StemmingTokenizer::new(),
            k1: self.k1,
            b: self.b,
            threshold: self.threshold,
            inverted: HashMap::new(),
            documents: HashMap::new(),
            doc_count: 0,
            total_doc_length: 0,
        })
    }
}

/// Document data stored for BM25 scoring.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentData {
    /// Term frequencies: term -> count
    pub term_freqs: HashMap<String, u32>,
    /// Document length (token count)
    pub doc_length: u32,
}

/// BM25 full-text search index.
///
/// Owns a `StemmingTokenizer` for both indexing and query tokenization.
/// Implements `Scorer` for integration with RRF fusion.
pub struct BM25Index {
    /// Internal stemming tokenizer — always StemmingTokenizer.
    #[allow(dead_code)]
    tokenizer: StemmingTokenizer,
    /// Term frequency saturation parameter.
    k1: f32,
    /// Document length normalization parameter.
    b: f32,
    /// Minimum score threshold.
    threshold: f32,
    /// term -> [(section_path, term_frequency)]
    inverted: HashMap<String, Vec<(VaultPath, u32)>>,
    /// section_path -> DocumentData
    documents: HashMap<VaultPath, DocumentData>,
    /// Total document count
    doc_count: usize,
    /// Sum of all document lengths (for avgdl calculation)
    total_doc_length: u64,
}

impl BM25Index {
    /// Add a document to the index from raw text.
    ///
    /// Tokenizes internally using the stemming tokenizer.
    /// Returns the token count for the document.
    pub fn add_document(&mut self, section_path: VaultPath, text: &str) -> usize {
        // Remove existing document if present
        self.remove_document(&section_path);

        let tokens = self.tokenizer.tokenize(text);
        let doc_length = tokens.len() as u32;

        // Count term frequencies
        let mut term_freqs: HashMap<String, u32> = HashMap::new();
        for token in &tokens {
            *term_freqs.entry(token.clone()).or_default() += 1;
        }

        // Update inverted index
        for (term, freq) in &term_freqs {
            self.inverted
                .entry(term.clone())
                .or_default()
                .push((section_path.clone(), *freq));
        }

        // Store document data
        self.documents.insert(
            section_path,
            DocumentData {
                term_freqs,
                doc_length,
            },
        );

        self.doc_count += 1;
        self.total_doc_length += doc_length as u64;

        tokens.len()
    }

    /// Remove a document from the index.
    pub fn remove_document(&mut self, section_path: &VaultPath) {
        if let Some(doc_data) = self.documents.remove(section_path) {
            self.doc_count = self.doc_count.saturating_sub(1);
            self.total_doc_length = self
                .total_doc_length
                .saturating_sub(doc_data.doc_length as u64);

            for term in doc_data.term_freqs.keys() {
                if let Some(postings) = self.inverted.get_mut(term) {
                    postings.retain(|(id, _)| id != section_path);
                    if postings.is_empty() {
                        self.inverted.remove(term);
                    }
                }
            }
        }
    }

    /// BM25 scoring for given query tokens against candidate documents.
    fn score_tokens(
        &self,
        query_tokens: &[String],
        candidates: &HashSet<VaultPath>,
    ) -> Vec<(VaultPath, f32)> {
        if query_tokens.is_empty() || self.doc_count == 0 {
            return Vec::new();
        }

        let avgdl = self.total_doc_length as f32 / self.doc_count as f32;
        let mut scores: HashMap<VaultPath, f32> = HashMap::new();

        for term in query_tokens {
            if let Some(postings) = self.inverted.get(term) {
                let n = postings.len() as f32;
                let idf = ((self.doc_count as f32 - n + 0.5) / (n + 0.5) + 1.0).ln();

                for (section_path, tf) in postings {
                    if !candidates.contains(section_path) {
                        continue;
                    }

                    let doc_data = self.documents.get(section_path).unwrap();
                    let dl = doc_data.doc_length as f32;
                    let tf = *tf as f32;
                    let term_score = idf * (tf * (self.k1 + 1.0))
                        / (tf + self.k1 * (1.0 - self.b + self.b * dl / avgdl));

                    *scores.entry(section_path.clone()).or_default() += term_score;
                }
            }
        }

        // Filter by threshold and sort
        let mut results: Vec<_> = scores
            .into_iter()
            .filter(|(_, score)| *score > self.threshold)
            .collect();
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results
    }

    /// Clear all indexed data.
    pub fn clear(&mut self) {
        self.inverted.clear();
        self.documents.clear();
        self.doc_count = 0;
        self.total_doc_length = 0;
    }

    /// Check if a section is indexed.
    pub fn contains(&self, section_path: &VaultPath) -> bool {
        self.documents.contains_key(section_path)
    }

    /// Get the document length (token count) for a section.
    pub fn doc_length(&self, section_path: &VaultPath) -> Option<u32> {
        self.documents.get(section_path).map(|d| d.doc_length)
    }
}

impl Scorer for BM25Index {
    fn score(&self, query: &str, candidates: &HashSet<VaultPath>) -> Vec<(VaultPath, f32)> {
        if query.is_empty() || candidates.is_empty() {
            return Vec::new();
        }
        let tokens = self.tokenizer.tokenize(query);
        self.score_tokens(&tokens, candidates)
    }
}

impl Configurable for BM25Index {
    type Config = BM25Config;

    fn config(&self) -> Self::Config {
        BM25Config {
            k1: self.k1,
            b: self.b,
            threshold: self.threshold,
        }
    }
}

impl std::fmt::Debug for BM25Index {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BM25Index")
            .field("k1", &self.k1)
            .field("b", &self.b)
            .field("threshold", &self.threshold)
            .field("doc_count", &self.doc_count)
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

    fn make_index() -> BM25Index {
        BM25Config::default().build().unwrap()
    }

    #[test]
    fn add_and_search() {
        let mut index = make_index();

        let s1 = section_path("rust-guide.md", &["Introduction"]);
        let s2 = section_path("python-guide.md", &["Introduction"]);
        let s3 = section_path("rust-web.md", &["Actix"]);

        index.add_document(
            s1.clone(),
            "Rust is a systems programming language focused on safety.",
        );
        index.add_document(s2.clone(), "Python is a dynamic programming language.");
        index.add_document(s3.clone(), "Actix is a web framework for Rust.");

        let all: HashSet<VaultPath> = [s1.clone(), s2.clone(), s3.clone()].into();
        let results = index.score("rust programming", &all);

        assert!(!results.is_empty());
        assert_eq!(results[0].0, s1);
    }

    #[test]
    fn score_respects_candidates() {
        let mut index = make_index();

        let s1 = section_path("note1.md", &[]);
        let s2 = section_path("note2.md", &[]);

        index.add_document(s1.clone(), "Rust programming language");
        index.add_document(s2.clone(), "Rust web framework");

        // Only s2 is a candidate
        let candidates: HashSet<VaultPath> = [s2.clone()].into();
        let results = index.score("rust", &candidates);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, s2);
    }

    #[test]
    fn remove_document() {
        let mut index = make_index();

        let s1 = section_path("note.md", &[]);
        index.add_document(s1.clone(), "Rust programming language");

        assert!(index.contains(&s1));

        index.remove_document(&s1);

        assert!(!index.contains(&s1));
        let all: HashSet<VaultPath> = [s1].into();
        assert!(index.score("rust", &all).is_empty());
    }

    #[test]
    fn clear_removes_all() {
        let mut index = make_index();

        let s1 = section_path("note.md", &[]);
        index.add_document(s1.clone(), "Rust programming");

        index.clear();

        assert!(!index.contains(&s1));
    }

    #[test]
    fn empty_query_returns_empty() {
        let mut index = make_index();

        let s1 = section_path("note.md", &[]);
        index.add_document(s1.clone(), "Some content");

        let all: HashSet<VaultPath> = [s1].into();
        assert!(index.score("", &all).is_empty());
    }

    #[test]
    fn empty_candidates_returns_empty() {
        let mut index = make_index();

        let s1 = section_path("note.md", &[]);
        index.add_document(s1, "Rust programming");

        let empty: HashSet<VaultPath> = HashSet::new();
        assert!(index.score("rust", &empty).is_empty());
    }

    #[test]
    fn threshold_filters_low_scores() {
        let config = BM25Config {
            threshold: 100.0, // Very high threshold
            ..Default::default()
        };
        let mut index = config.build().unwrap();

        let s1 = section_path("note.md", &[]);
        index.add_document(s1.clone(), "Rust programming");

        let all: HashSet<VaultPath> = [s1].into();
        assert!(index.score("rust", &all).is_empty());
    }

    #[test]
    fn add_document_returns_token_count() {
        let mut index = make_index();
        let s1 = section_path("note.md", &[]);
        let count = index.add_document(s1, "hello world foo");
        assert!(count > 0);
    }

    #[test]
    fn config_roundtrip() {
        let config = BM25Config {
            k1: 1.5,
            b: 0.8,
            threshold: 0.1,
        };
        let index = config.build().unwrap();
        assert_eq!(index.config(), config);
    }

    #[test]
    fn default_config() {
        let config = BM25Config::default();
        assert!((config.k1 - 1.2).abs() < f32::EPSILON);
        assert!((config.b - 0.75).abs() < f32::EPSILON);
        assert!((config.threshold - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn update_document_replaces_content() {
        let mut index = make_index();

        let s1 = section_path("note.md", &[]);

        index.add_document(s1.clone(), "Python programming");
        let all: HashSet<VaultPath> = [s1.clone()].into();
        assert!(!index.score("python", &all).is_empty());
        assert!(index.score("rust", &all).is_empty());

        index.add_document(s1, "Rust programming");
        let all: HashSet<VaultPath> = [section_path("note.md", &[])].into();
        assert!(!index.score("rust", &all).is_empty());
        assert!(index.score("python", &all).is_empty());
    }
}
