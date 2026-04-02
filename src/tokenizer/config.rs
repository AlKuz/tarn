use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::common::Buildable;
use crate::tokenizer::Tokenizer;

#[derive(Debug, Error)]
pub enum TokenizerError {
    #[error("feature '{0}' is not enabled")]
    FeatureNotEnabled(String),
    #[error("failed to load tokenizer: {0}")]
    LoadFailed(String),
    #[error(transparent)]
    Ngram(#[from] super::NgramError),
}

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
