//! BM25 full-text search index with pluggable tokenizers.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::SectionId;
use crate::common::Buildable;
use crate::tokenizer::{Tokenizer, TokenizerConfig};

/// BM25 algorithm parameters.
const K1: f32 = 1.2; // Term frequency saturation
const B: f32 = 0.75; // Document length normalization

/// Errors from BM25 index operations.
#[derive(Debug, Error)]
pub enum BM25Error {
    #[error("tokenizer error: {0}")]
    Tokenizer(#[from] crate::tokenizer::TokenizerError),
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
/// Generic over `T: Tokenizer` for text processing. Stores inverted index
/// for efficient term lookup and document data for BM25 scoring.
#[derive(Debug, Serialize, Deserialize)]
#[serde(bound(
    serialize = "T: Serialize",
    deserialize = "T: serde::de::DeserializeOwned"
))]
pub struct BM25Index<T: Tokenizer> {
    /// term -> [(section_id, term_frequency)]
    inverted: HashMap<String, Vec<(SectionId, u32)>>,
    /// section_id -> DocumentData
    documents: HashMap<SectionId, DocumentData>,
    /// Total document count
    doc_count: usize,
    /// Sum of all document lengths (for avgdl calculation)
    total_doc_length: u64,
    /// Tokenizer instance
    tokenizer: T,
}

impl<T: Tokenizer> BM25Index<T> {
    pub fn new(tokenizer: T) -> Self {
        Self {
            inverted: HashMap::new(),
            documents: HashMap::new(),
            doc_count: 0,
            total_doc_length: 0,
            tokenizer,
        }
    }

    /// Add a document to the index.
    pub fn add_document(&mut self, section_id: SectionId, content: &str) {
        // Remove existing document if present
        self.remove_document(&section_id);

        let tokens = self.tokenize(content);
        let doc_length = tokens.len() as u32;

        // Count term frequencies
        let mut term_freqs: HashMap<String, u32> = HashMap::new();
        for token in tokens {
            *term_freqs.entry(token).or_default() += 1;
        }

        // Update inverted index
        for (term, freq) in &term_freqs {
            self.inverted
                .entry(term.clone())
                .or_default()
                .push((section_id.clone(), *freq));
        }

        // Store document data
        self.documents.insert(
            section_id,
            DocumentData {
                term_freqs,
                doc_length,
            },
        );

        self.doc_count += 1;
        self.total_doc_length += doc_length as u64;
    }

    /// Remove a document from the index.
    pub fn remove_document(&mut self, section_id: &SectionId) {
        if let Some(doc_data) = self.documents.remove(section_id) {
            // Update statistics
            self.doc_count = self.doc_count.saturating_sub(1);
            self.total_doc_length = self
                .total_doc_length
                .saturating_sub(doc_data.doc_length as u64);

            // Remove from inverted index
            for term in doc_data.term_freqs.keys() {
                if let Some(postings) = self.inverted.get_mut(term) {
                    postings.retain(|(id, _)| id != section_id);
                    if postings.is_empty() {
                        self.inverted.remove(term);
                    }
                }
            }
        }
    }

    /// Search the index with BM25 scoring.
    ///
    /// Returns (section_id, score) pairs sorted by score descending.
    pub fn search(&self, query: &str, limit: usize) -> Vec<(SectionId, f32)> {
        let query_tokens = self.tokenize(query);
        if query_tokens.is_empty() || self.doc_count == 0 {
            return Vec::new();
        }

        let avgdl = self.total_doc_length as f32 / self.doc_count as f32;

        // Accumulate scores per document
        let mut scores: HashMap<SectionId, f32> = HashMap::new();

        for term in &query_tokens {
            if let Some(postings) = self.inverted.get(term) {
                // IDF: log((N - n + 0.5) / (n + 0.5) + 1)
                let n = postings.len() as f32;
                let idf = ((self.doc_count as f32 - n + 0.5) / (n + 0.5) + 1.0).ln();

                for (section_id, tf) in postings {
                    let doc_data = self.documents.get(section_id).unwrap();
                    let dl = doc_data.doc_length as f32;

                    // BM25 term score: IDF * (tf * (k1 + 1)) / (tf + k1 * (1 - b + b * dl/avgdl))
                    let tf = *tf as f32;
                    let term_score =
                        idf * (tf * (K1 + 1.0)) / (tf + K1 * (1.0 - B + B * dl / avgdl));

                    *scores.entry(section_id.clone()).or_default() += term_score;
                }
            }
        }

        // Sort by score descending
        let mut results: Vec<_> = scores.into_iter().collect();
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(limit);

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
    pub fn contains(&self, section_id: &SectionId) -> bool {
        self.documents.contains_key(section_id)
    }

    /// Get the document length (token count) for a section.
    pub fn doc_length(&self, section_id: &SectionId) -> Option<u32> {
        self.documents.get(section_id).map(|d| d.doc_length)
    }

    /// Tokenize text using the stored tokenizer.
    fn tokenize(&self, text: &str) -> Vec<String> {
        self.tokenizer.tokenize(text)
    }
}

impl BM25Index<Box<dyn Tokenizer>> {
    /// Create a new BM25 index from a tokenizer config.
    pub fn from_config(config: TokenizerConfig) -> Result<Self, BM25Error> {
        let tokenizer = config.build()?;
        Ok(Self::new(tokenizer))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::VaultPath;
    use crate::tokenizer::NaiveTokenizer;

    fn section_id(path: &str, headings: &[&str]) -> SectionId {
        let vault_path = VaultPath::new(path).unwrap();
        let heading_path: Vec<String> = headings.iter().map(|s| s.to_string()).collect();
        SectionId::new(&vault_path, &heading_path)
    }

    fn make_index() -> BM25Index<NaiveTokenizer> {
        BM25Index::new(NaiveTokenizer::new())
    }

    #[test]
    fn add_and_search() {
        let mut index = make_index();

        let s1 = section_id("rust-guide.md", &["Introduction"]);
        let s2 = section_id("python-guide.md", &["Introduction"]);
        let s3 = section_id("rust-web.md", &["Actix"]);

        index.add_document(
            s1.clone(),
            "Rust is a systems programming language focused on safety.",
        );
        index.add_document(s2.clone(), "Python is a dynamic programming language.");
        index.add_document(s3.clone(), "Actix is a web framework for Rust.");

        let results = index.search("rust programming", 10);

        assert!(!results.is_empty());
        assert_eq!(results[0].0, s1);
    }

    #[test]
    fn remove_document() {
        let mut index = make_index();

        let s1 = section_id("note.md", &[]);
        index.add_document(s1.clone(), "Rust programming language");

        assert!(index.contains(&s1));

        index.remove_document(&s1);

        assert!(!index.contains(&s1));
        assert!(index.search("rust", 10).is_empty());
    }

    #[test]
    fn clear_removes_all() {
        let mut index = make_index();

        let s1 = section_id("note.md", &[]);
        index.add_document(s1.clone(), "Rust programming");

        index.clear();

        assert!(!index.contains(&s1));
        assert!(index.search("rust", 10).is_empty());
    }

    #[test]
    fn empty_query_returns_empty() {
        let mut index = make_index();

        let s1 = section_id("note.md", &[]);
        index.add_document(s1, "Some content");

        assert!(index.search("", 10).is_empty());
    }

    #[test]
    fn search_respects_limit() {
        let mut index = make_index();

        for i in 0..10 {
            let s = section_id(&format!("note{i}.md"), &[]);
            index.add_document(s, "rust programming language");
        }

        let results = index.search("rust", 3);
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn bm25_scores_term_frequency() {
        let mut index = make_index();

        let s1 = section_id("note1.md", &[]);
        let s2 = section_id("note2.md", &[]);

        index.add_document(s1.clone(), "Rust is great.");
        index.add_document(s2.clone(), "Rust Rust Rust is the best Rust language.");

        let results = index.search("rust", 10);

        assert!(results.len() >= 2);
    }

    #[test]
    fn update_document_replaces_content() {
        let mut index = make_index();

        let s1 = section_id("note.md", &[]);

        index.add_document(s1.clone(), "Python programming");
        assert!(index.search("python", 10).len() == 1);
        assert!(index.search("rust", 10).is_empty());

        index.add_document(s1.clone(), "Rust programming");
        assert!(index.search("rust", 10).len() == 1);
        assert!(index.search("python", 10).is_empty());
    }

    #[test]
    fn doc_length_returns_token_count() {
        let mut index = make_index();

        let s1 = section_id("note.md", &[]);
        index.add_document(s1.clone(), "hello world foo");

        assert_eq!(index.doc_length(&s1), Some(3));
    }

    #[test]
    fn from_config_naive() {
        let mut index = BM25Index::from_config(TokenizerConfig::Naive).unwrap();
        let s1 = section_id("note.md", &[]);
        index.add_document(s1.clone(), "hello world");
        assert!(!index.search("hello", 10).is_empty());
    }

    #[cfg(feature = "hf-tokenizer")]
    #[test]
    fn from_config_hf() {
        let mut index = BM25Index::from_config(TokenizerConfig::HuggingFace {
            model_id: "bert-base-uncased".to_string(),
        })
        .unwrap();
        let s1 = section_id("note.md", &[]);
        index.add_document(s1.clone(), "Rust programming");
        assert!(!index.search("rust", 10).is_empty());
    }
}
