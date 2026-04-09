use serde::{Deserialize, Serialize};

use super::errors::TokenizerError;
use crate::common::Buildable;
use crate::tokenizer::Tokenizer;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TokenizerConfig {
    #[default]
    Naive,
    /// HuggingFace tokenizer (requires `hf-tokenizer` feature).
    HuggingFace { model_id: String },
    /// Language-aware stemming tokenizer (requires `stemming` feature).
    Stemming,
    /// Character n-gram tokenizer for trigram similarity scoring.
    Ngram {
        /// Size of character n-grams (default: 3).
        #[serde(default = "default_ngram_n")]
        n: usize,
    },
}

fn default_ngram_n() -> usize {
    3
}

impl Buildable for TokenizerConfig {
    type Target = Box<dyn Tokenizer>;
    type Error = TokenizerError;

    fn build(&self) -> Result<Self::Target, Self::Error> {
        use crate::tokenizer::NaiveTokenizer;

        match self {
            TokenizerConfig::Naive => Ok(Box::new(NaiveTokenizer::new())),
            #[cfg(feature = "hf-tokenizer")]
            TokenizerConfig::HuggingFace { model_id } => {
                let tokenizer = crate::tokenizer::HfTokenizer::new(model_id)?;
                Ok(Box::new(tokenizer))
            }
            #[cfg(not(feature = "hf-tokenizer"))]
            TokenizerConfig::HuggingFace { .. } => Err(TokenizerError::FeatureNotEnabled(
                "hf-tokenizer".to_string(),
            )),
            #[cfg(feature = "stemming")]
            TokenizerConfig::Stemming => {
                let tokenizer = crate::tokenizer::StemmingTokenizer::new();
                Ok(Box::new(tokenizer))
            }
            #[cfg(not(feature = "stemming"))]
            TokenizerConfig::Stemming => {
                Err(TokenizerError::FeatureNotEnabled("stemming".to_string()))
            }
            TokenizerConfig::Ngram { n } => {
                let tokenizer = crate::tokenizer::NgramTokenizer::new(*n)?;
                Ok(Box::new(tokenizer))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::Buildable;

    #[test]
    fn default_variant_is_naive() {
        assert_eq!(TokenizerConfig::default(), TokenizerConfig::Naive);
    }

    #[test]
    fn build_naive() {
        let tokenizer = TokenizerConfig::Naive.build().unwrap();
        let tokens = tokenizer.tokenize("hello world");
        assert!(!tokens.is_empty());
    }

    #[cfg(feature = "stemming")]
    #[test]
    fn build_stemming() {
        let tokenizer = TokenizerConfig::Stemming.build().unwrap();
        let tokens = tokenizer.tokenize("running quickly");
        assert!(!tokens.is_empty());
    }

    #[test]
    fn build_ngram() {
        let tokenizer = TokenizerConfig::Ngram { n: 3 }.build().unwrap();
        let tokens = tokenizer.tokenize("hello");
        assert!(!tokens.is_empty());
    }

    #[cfg(not(feature = "hf-tokenizer"))]
    #[test]
    fn build_hf_without_feature() {
        let result = TokenizerConfig::HuggingFace {
            model_id: "test".to_string(),
        }
        .build();
        assert!(result.is_err());
    }

    #[test]
    fn ngram_serde_default_n() {
        let config: TokenizerConfig = serde_json::from_str(r#"{"type": "ngram"}"#).unwrap();
        assert_eq!(config, TokenizerConfig::Ngram { n: 3 });
    }
}
