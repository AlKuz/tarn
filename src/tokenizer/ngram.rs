//! Character n-gram tokenizer for trigram similarity scoring.

use serde::{Deserialize, Serialize};

use crate::common::{Buildable, Configurable};

/// Configuration for the n-gram tokenizer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NgramTokenizerConfig {
    /// Size of character n-grams (default: 3).
    #[serde(default = "default_n")]
    pub n: usize,
}

fn default_n() -> usize {
    3
}

impl Default for NgramTokenizerConfig {
    fn default() -> Self {
        Self { n: default_n() }
    }
}

impl Buildable for NgramTokenizerConfig {
    type Target = NgramTokenizer;
    type Error = std::convert::Infallible;

    fn build(&self) -> Result<Self::Target, Self::Error> {
        Ok(NgramTokenizer::new(self.n))
    }
}

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
    pub fn new(n: usize) -> Self {
        assert!(n > 0, "n-gram size must be positive");
        Self { n }
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
    type Config = NgramTokenizerConfig;

    fn config(&self) -> Self::Config {
        NgramTokenizerConfig { n: self.n }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trigrams_basic() {
        let tok = NgramTokenizer::new(3);
        let ngrams = tok.tokenize("rust");
        // padded: "$$rust$$"
        // windows: "$$r", "$ru", "rus", "ust", "st$", "t$$"
        assert_eq!(ngrams, vec!["$$r", "$ru", "rus", "ust", "st$", "t$$"]);
    }

    #[test]
    fn trigrams_lowercase() {
        let tok = NgramTokenizer::new(3);
        let ngrams = tok.tokenize("Rust");
        assert_eq!(ngrams, vec!["$$r", "$ru", "rus", "ust", "st$", "t$$"]);
    }

    #[test]
    fn empty_string() {
        let tok = NgramTokenizer::new(3);
        let ngrams = tok.tokenize("");
        // padded: "$$$$" (4 chars), windows of 3: ["$$$", "$$$"]
        // This is degenerate but should not panic
        assert!(!ngrams.is_empty());
    }

    #[test]
    fn single_char() {
        let tok = NgramTokenizer::new(3);
        let ngrams = tok.tokenize("a");
        // padded: "$$a$$"
        // windows: "$$a", "$a$", "a$$"
        assert_eq!(ngrams, vec!["$$a", "$a$", "a$$"]);
    }

    #[test]
    fn bigrams() {
        let tok = NgramTokenizer::new(2);
        let ngrams = tok.tokenize("ab");
        // padded: "$ab$"
        // windows: "$a", "ab", "b$"
        assert_eq!(ngrams, vec!["$a", "ab", "b$"]);
    }

    #[test]
    fn spaces_preserved() {
        let tok = NgramTokenizer::new(3);
        let ngrams = tok.tokenize("a b");
        // padded: "$$a b$$"
        assert!(ngrams.contains(&"a b".to_string()));
    }

    #[test]
    fn config_roundtrip() {
        let config = NgramTokenizerConfig { n: 4 };
        let tok = config.build().unwrap();
        assert_eq!(tok.config(), NgramTokenizerConfig { n: 4 });
    }

    #[test]
    fn default_config() {
        let config = NgramTokenizerConfig::default();
        assert_eq!(config.n, 3);
    }

    #[test]
    #[should_panic(expected = "n-gram size must be positive")]
    fn zero_n_panics() {
        NgramTokenizer::new(0);
    }
}
