//! Simple whitespace-based tokenizer.

use super::{Tokenizer, TokenizerConfig};
use crate::common::Configurable;

/// Simple whitespace-based tokenizer.
///
/// Splits text on whitespace, removes punctuation, and lowercases all tokens.
/// Does not perform stemming, stop word removal, or other NLP processing.
#[derive(Debug, Clone, Default)]
pub struct NaiveTokenizer;

impl NaiveTokenizer {
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

impl Configurable for NaiveTokenizer {
    type Config = TokenizerConfig;

    fn config(&self) -> Self::Config {
        TokenizerConfig::Naive
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_on_whitespace() {
        let tokenizer = NaiveTokenizer::new();
        let tokens = tokenizer.tokenize("hello world");
        assert_eq!(tokens, vec!["hello", "world"]);
    }

    #[test]
    fn lowercases() {
        let tokenizer = NaiveTokenizer::new();
        let tokens = tokenizer.tokenize("Hello WORLD");
        assert_eq!(tokens, vec!["hello", "world"]);
    }

    #[test]
    fn handles_multiple_whitespace() {
        let tokenizer = NaiveTokenizer::new();
        let tokens = tokenizer.tokenize("hello   world\t\nfoo");
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn empty_input() {
        let tokenizer = NaiveTokenizer::new();
        let tokens = tokenizer.tokenize("");
        assert!(tokens.is_empty());
    }

    #[test]
    fn whitespace_only() {
        let tokenizer = NaiveTokenizer::new();
        let tokens = tokenizer.tokenize("   \t\n  ");
        assert!(tokens.is_empty());
    }

    #[test]
    fn removes_punctuation() {
        let tokenizer = NaiveTokenizer::new();
        let tokens = tokenizer.tokenize("Hello, world! How's it going?");
        assert_eq!(tokens, vec!["hello", "world", "hows", "it", "going"]);
    }

    #[test]
    fn punctuation_only_tokens() {
        let tokenizer = NaiveTokenizer::new();
        let tokens = tokenizer.tokenize("hello ... world");
        assert_eq!(tokens, vec!["hello", "world"]);
    }

    #[test]
    fn get_config_returns_naive() {
        let tokenizer = NaiveTokenizer::new();
        assert!(matches!(tokenizer.config(), TokenizerConfig::Naive));
    }
}
