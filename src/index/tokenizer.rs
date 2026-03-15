//! Tokenizer for BM25 full-text search.
//!
//! Converts note content into searchable tokens for indexing and query matching.

// ---------------------------------------------------------------------------
// Tokenizer trait
// ---------------------------------------------------------------------------

/// Trait for tokenizing text into searchable tokens.
///
/// Implementations convert input text into normalized token sequences suitable
/// for BM25 indexing. This is a synchronous interface since tokenization is
/// CPU-bound with no I/O.
pub trait Tokenizer: Send + Sync {
    /// Tokenize text into a sequence of normalized tokens.
    ///
    /// # Arguments
    ///
    /// * `text` - The input text to tokenize
    ///
    /// # Returns
    ///
    /// A vector of normalized tokens extracted from the text.
    fn tokenize(&self, text: &str) -> Vec<String>;
}

// ---------------------------------------------------------------------------
// NaiveTokenizer
// ---------------------------------------------------------------------------

/// Simple whitespace-based tokenizer.
///
/// Splits text on whitespace, removes punctuation, and lowercases all tokens.
/// Does not perform stemming, stop word removal, or other NLP processing.
#[derive(Debug, Clone, Default)]
pub struct NaiveTokenizer;

impl NaiveTokenizer {
    /// Create a new naive tokenizer.
    pub fn new() -> Self {
        Self
    }
}

impl Tokenizer for NaiveTokenizer {
    fn tokenize(&self, text: &str) -> Vec<String> {
        text.split_whitespace()
            .map(|s| {
                s.chars()
                    .filter(|c| c.is_alphanumeric())
                    .collect::<String>()
                    .to_lowercase()
            })
            .filter(|s| !s.is_empty())
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn naive_tokenizer_splits_on_whitespace() {
        let tokenizer = NaiveTokenizer::new();
        let tokens = tokenizer.tokenize("hello world");
        assert_eq!(tokens, vec!["hello", "world"]);
    }

    #[test]
    fn naive_tokenizer_lowercases() {
        let tokenizer = NaiveTokenizer::new();
        let tokens = tokenizer.tokenize("Hello WORLD");
        assert_eq!(tokens, vec!["hello", "world"]);
    }

    #[test]
    fn naive_tokenizer_handles_multiple_whitespace() {
        let tokenizer = NaiveTokenizer::new();
        let tokens = tokenizer.tokenize("hello   world\t\nfoo");
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn naive_tokenizer_empty_input() {
        let tokenizer = NaiveTokenizer::new();
        let tokens = tokenizer.tokenize("");
        assert!(tokens.is_empty());
    }

    #[test]
    fn naive_tokenizer_whitespace_only() {
        let tokenizer = NaiveTokenizer::new();
        let tokens = tokenizer.tokenize("   \t\n  ");
        assert!(tokens.is_empty());
    }

    #[test]
    fn naive_tokenizer_removes_punctuation() {
        let tokenizer = NaiveTokenizer::new();
        let tokens = tokenizer.tokenize("Hello, world! How's it going?");
        assert_eq!(tokens, vec!["hello", "world", "hows", "it", "going"]);
    }

    #[test]
    fn naive_tokenizer_punctuation_only_tokens() {
        let tokenizer = NaiveTokenizer::new();
        let tokens = tokenizer.tokenize("hello ... world");
        assert_eq!(tokens, vec!["hello", "world"]);
    }
}
