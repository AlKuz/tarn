//! BM25 full-text search index.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::common::VaultPath;

/// BM25 algorithm parameters.
const K1: f32 = 1.2; // Term frequency saturation
const B: f32 = 0.75; // Document length normalization

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
/// Pure data structure for term-based scoring. Tokenization is performed
/// externally before calling `add_document` or `search`.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct BM25Index {
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
    pub fn new() -> Self {
        Self {
            inverted: HashMap::new(),
            documents: HashMap::new(),
            doc_count: 0,
            total_doc_length: 0,
        }
    }

    /// Add a document to the index from pre-tokenized input.
    pub fn add_document(&mut self, section_path: VaultPath, tokens: &[String]) {
        // Remove existing document if present
        self.remove_document(&section_path);

        let doc_length = tokens.len() as u32;

        // Count term frequencies
        let mut term_freqs: HashMap<String, u32> = HashMap::new();
        for token in tokens {
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
    }

    /// Remove a document from the index.
    pub fn remove_document(&mut self, section_path: &VaultPath) {
        if let Some(doc_data) = self.documents.remove(section_path) {
            // Update statistics
            self.doc_count = self.doc_count.saturating_sub(1);
            self.total_doc_length = self
                .total_doc_length
                .saturating_sub(doc_data.doc_length as u64);

            // Remove from inverted index
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

    /// Search the index with BM25 scoring from pre-tokenized query.
    ///
    /// Returns (section_path, score) pairs sorted by score descending.
    pub fn search(&self, query_tokens: &[String], limit: usize) -> Vec<(VaultPath, f32)> {
        if query_tokens.is_empty() || self.doc_count == 0 {
            return Vec::new();
        }

        let avgdl = self.total_doc_length as f32 / self.doc_count as f32;

        // Accumulate scores per document
        let mut scores: HashMap<VaultPath, f32> = HashMap::new();

        for term in query_tokens {
            if let Some(postings) = self.inverted.get(term) {
                // IDF: log((N - n + 0.5) / (n + 0.5) + 1)
                let n = postings.len() as f32;
                let idf = ((self.doc_count as f32 - n + 0.5) / (n + 0.5) + 1.0).ln();

                for (section_path, tf) in postings {
                    let doc_data = self.documents.get(section_path).unwrap();
                    let dl = doc_data.doc_length as f32;

                    // BM25 term score: IDF * (tf * (k1 + 1)) / (tf + k1 * (1 - b + b * dl/avgdl))
                    let tf = *tf as f32;
                    let term_score =
                        idf * (tf * (K1 + 1.0)) / (tf + K1 * (1.0 - B + B * dl / avgdl));

                    *scores.entry(section_path.clone()).or_default() += term_score;
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
    pub fn contains(&self, section_path: &VaultPath) -> bool {
        self.documents.contains_key(section_path)
    }

    /// Get the document length (token count) for a section.
    pub fn doc_length(&self, section_path: &VaultPath) -> Option<u32> {
        self.documents.get(section_path).map(|d| d.doc_length)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokenizer::{NaiveTokenizer, Tokenizer};

    fn section_path(path: &str, headings: &[&str]) -> VaultPath {
        let heading_path = headings.join("/");
        VaultPath::new(format!("{path}#{heading_path}")).unwrap()
    }

    fn tokenize(text: &str) -> Vec<String> {
        NaiveTokenizer::new().tokenize(text)
    }

    fn make_index() -> BM25Index {
        BM25Index::new()
    }

    #[test]
    fn add_and_search() {
        let mut index = make_index();

        let s1 = section_path("rust-guide.md", &["Introduction"]);
        let s2 = section_path("python-guide.md", &["Introduction"]);
        let s3 = section_path("rust-web.md", &["Actix"]);

        index.add_document(
            s1.clone(),
            &tokenize("Rust is a systems programming language focused on safety."),
        );
        index.add_document(
            s2.clone(),
            &tokenize("Python is a dynamic programming language."),
        );
        index.add_document(s3.clone(), &tokenize("Actix is a web framework for Rust."));

        let results = index.search(&tokenize("rust programming"), 10);

        assert!(!results.is_empty());
        assert_eq!(results[0].0, s1);
    }

    #[test]
    fn remove_document() {
        let mut index = make_index();

        let s1 = section_path("note.md", &[]);
        index.add_document(s1.clone(), &tokenize("Rust programming language"));

        assert!(index.contains(&s1));

        index.remove_document(&s1);

        assert!(!index.contains(&s1));
        assert!(index.search(&tokenize("rust"), 10).is_empty());
    }

    #[test]
    fn clear_removes_all() {
        let mut index = make_index();

        let s1 = section_path("note.md", &[]);
        index.add_document(s1.clone(), &tokenize("Rust programming"));

        index.clear();

        assert!(!index.contains(&s1));
        assert!(index.search(&tokenize("rust"), 10).is_empty());
    }

    #[test]
    fn empty_query_returns_empty() {
        let mut index = make_index();

        let s1 = section_path("note.md", &[]);
        index.add_document(s1, &tokenize("Some content"));

        assert!(index.search(&tokenize(""), 10).is_empty());
    }

    #[test]
    fn search_respects_limit() {
        let mut index = make_index();

        for i in 0..10 {
            let s = section_path(&format!("note{i}.md"), &[]);
            index.add_document(s, &tokenize("rust programming language"));
        }

        let results = index.search(&tokenize("rust"), 3);
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn bm25_scores_term_frequency() {
        let mut index = make_index();

        let s1 = section_path("note1.md", &[]);
        let s2 = section_path("note2.md", &[]);

        index.add_document(s1.clone(), &tokenize("Rust is great."));
        index.add_document(
            s2.clone(),
            &tokenize("Rust Rust Rust is the best Rust language."),
        );

        let results = index.search(&tokenize("rust"), 10);

        assert!(results.len() >= 2);
    }

    #[test]
    fn update_document_replaces_content() {
        let mut index = make_index();

        let s1 = section_path("note.md", &[]);

        index.add_document(s1.clone(), &tokenize("Python programming"));
        assert!(index.search(&tokenize("python"), 10).len() == 1);
        assert!(index.search(&tokenize("rust"), 10).is_empty());

        index.add_document(s1.clone(), &tokenize("Rust programming"));
        assert!(index.search(&tokenize("rust"), 10).len() == 1);
        assert!(index.search(&tokenize("python"), 10).is_empty());
    }

    #[test]
    fn doc_length_returns_token_count() {
        let mut index = make_index();

        let s1 = section_path("note.md", &[]);
        index.add_document(s1.clone(), &tokenize("hello world foo"));

        assert_eq!(index.doc_length(&s1), Some(3));
    }
}
