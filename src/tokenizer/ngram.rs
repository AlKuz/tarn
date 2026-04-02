//! Character n-gram tokenizer for trigram similarity scoring.

use super::config::TokenizerConfig;
use super::errors::TokenizerError;
use crate::common::Configurable;

/// Character n-gram tokenizer.
///
/// Generates character-level n-grams from text. Pads with `$` on both sides
/// to capture start/end context. Used internally by `TagIndex` for trigram
/// similarity scoring.
#[derive(Debug, Clone)]
pub struct NgramTokenizer {
    n: usize,
}

impl NgramTokenizer {
    pub fn new(n: usize) -> Result<Self, TokenizerError> {
        if n == 0 {
            return Err(TokenizerError::InvalidNgramSize(n));
        }
        Ok(Self { n })
    }

    /// Generate n-grams from text.
    ///
    /// Lowercases the input, pads with `$`, and produces sliding windows.
    pub fn tokenize(&self, text: &str) -> Vec<String> {
        let lower = text.to_lowercase();
        let padded = format!(
            "{}{}{}",
            "$".repeat(self.n - 1),
            lower,
            "$".repeat(self.n - 1)
        );
        let chars: Vec<char> = padded.chars().collect();

        if chars.len() < self.n {
            return Vec::new();
        }

        chars
            .windows(self.n)
            .map(|w| w.iter().collect::<String>())
            .collect()
    }
}

impl Configurable for NgramTokenizer {
    type Config = TokenizerConfig;

    fn config(&self) -> Self::Config {
        TokenizerConfig::Ngram { n: self.n }
    }
}

impl super::Tokenizer for NgramTokenizer {
    fn tokenize(&self, text: &str) -> Vec<String> {
        self.tokenize(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trigrams_basic() {
        let tok = NgramTokenizer::new(3).unwrap();
        let ngrams = tok.tokenize("rust");
        assert_eq!(ngrams, vec!["$$r", "$ru", "rus", "ust", "st$", "t$$"]);
    }

    #[test]
    fn trigrams_lowercase() {
        let tok = NgramTokenizer::new(3).unwrap();
        let ngrams = tok.tokenize("Rust");
        assert_eq!(ngrams, vec!["$$r", "$ru", "rus", "ust", "st$", "t$$"]);
    }

    #[test]
    fn empty_string() {
        let tok = NgramTokenizer::new(3).unwrap();
        let ngrams = tok.tokenize("");
        assert!(!ngrams.is_empty());
    }

    #[test]
    fn single_char() {
        let tok = NgramTokenizer::new(3).unwrap();
        let ngrams = tok.tokenize("a");
        assert_eq!(ngrams, vec!["$$a", "$a$", "a$$"]);
    }

    #[test]
    fn bigrams() {
        let tok = NgramTokenizer::new(2).unwrap();
        let ngrams = tok.tokenize("ab");
        assert_eq!(ngrams, vec!["$a", "ab", "b$"]);
    }

    #[test]
    fn spaces_preserved() {
        let tok = NgramTokenizer::new(3).unwrap();
        let ngrams = tok.tokenize("a b");
        assert!(ngrams.contains(&"a b".to_string()));
    }

    #[test]
    fn config_roundtrip() {
        let tok = NgramTokenizer::new(4).unwrap();
        assert_eq!(tok.config(), TokenizerConfig::Ngram { n: 4 });
    }

    #[test]
    fn zero_n_returns_error() {
        assert!(NgramTokenizer::new(0).is_err());
    }
}
