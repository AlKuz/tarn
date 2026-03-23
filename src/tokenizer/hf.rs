//! HuggingFace tokenizer implementation.

use super::{Tokenizer, TokenizerConfig, TokenizerError};
use crate::common::Configurable;

/// HuggingFace tokenizer using pretrained models.
///
/// Wraps the `tokenizers` crate to provide subword tokenization
/// (e.g., WordPiece, BPE) suitable for BM25 indexing.
pub struct HfTokenizer {
    tokenizer: tokenizers::Tokenizer,
    model_id: String,
}

impl std::fmt::Debug for HfTokenizer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HfTokenizer")
            .field("model_id", &self.model_id)
            .finish()
    }
}

impl HfTokenizer {
    /// Create a new HuggingFace tokenizer from a pretrained model.
    ///
    /// # Arguments
    ///
    /// * `model_id` - HuggingFace model ID (e.g., "bert-base-uncased")
    pub fn new(model_id: &str) -> Result<Self, TokenizerError> {
        let tokenizer = tokenizers::Tokenizer::from_pretrained(model_id, None)
            .map_err(|e| TokenizerError::LoadFailed(format!("{model_id}: {e}")))?;
        Ok(Self {
            tokenizer,
            model_id: model_id.to_string(),
        })
    }
}

impl Tokenizer for HfTokenizer {
    fn tokenize(&self, text: &str) -> Vec<String> {
        let encoding = match self.tokenizer.encode(text, false) {
            Ok(e) => e,
            Err(_) => return Vec::new(),
        };

        encoding
            .get_tokens()
            .iter()
            .filter(|t| !is_special_token(t))
            .map(|t| normalize_token(t))
            .filter(|t| !t.is_empty())
            .collect()
    }
}

impl Configurable for HfTokenizer {
    type Config = TokenizerConfig;

    fn config(&self) -> Self::Config {
        TokenizerConfig::HuggingFace {
            model_id: self.model_id.clone(),
        }
    }
}

/// Check if a token is a special token (e.g., [CLS], [SEP], [PAD]).
fn is_special_token(token: &str) -> bool {
    token.starts_with('[') && token.ends_with(']')
}

/// Normalize a token for consistent matching.
fn normalize_token(token: &str) -> String {
    // Handle WordPiece continuation markers (##)
    token.trim_start_matches("##").to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenizes_basic_text() {
        let tokenizer = HfTokenizer::new("bert-base-uncased").unwrap();
        let tokens = tokenizer.tokenize("Hello world");
        assert!(!tokens.is_empty());
        assert!(tokens.contains(&"hello".to_string()));
        assert!(tokens.contains(&"world".to_string()));
    }

    #[test]
    fn filters_special_tokens() {
        assert!(is_special_token("[CLS]"));
        assert!(is_special_token("[SEP]"));
        assert!(is_special_token("[PAD]"));
        assert!(!is_special_token("hello"));
        assert!(!is_special_token("[incomplete"));
    }

    #[test]
    fn normalizes_wordpiece_tokens() {
        assert_eq!(normalize_token("##ing"), "ing");
        assert_eq!(normalize_token("Hello"), "hello");
        assert_eq!(normalize_token("##ED"), "ed");
    }

    #[test]
    fn empty_input() {
        let tokenizer = HfTokenizer::new("bert-base-uncased").unwrap();
        let tokens = tokenizer.tokenize("");
        assert!(tokens.is_empty());
    }

    #[test]
    fn get_config_returns_hf() {
        let tokenizer = HfTokenizer::new("bert-base-uncased").unwrap();
        match tokenizer.config() {
            TokenizerConfig::HuggingFace { model_id } => {
                assert_eq!(model_id, "bert-base-uncased");
            }
            _ => panic!("expected HuggingFace config"),
        }
    }

    #[test]
    fn invalid_model_returns_error() {
        let result = HfTokenizer::new("nonexistent-model-xyz-12345");
        assert!(result.is_err());
    }
}
