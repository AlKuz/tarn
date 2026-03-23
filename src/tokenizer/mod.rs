//! Tokenizer module for text processing.
//!
//! Provides a trait-based tokenizer abstraction with feature-gated implementations:
//! - `NaiveTokenizer` — always available, whitespace-based
//! - `HfTokenizer` — HuggingFace tokenizers (requires `hf-tokenizer` feature)
//! - `StemmingTokenizer` — language-aware stemming (requires `stemming` feature)

#[cfg(feature = "hf-tokenizer")]
mod hf;
mod naive;
#[cfg(feature = "stemming")]
mod stemming;

#[cfg(feature = "hf-tokenizer")]
pub use hf::HfTokenizer;
pub use naive::NaiveTokenizer;
#[cfg(feature = "stemming")]
pub use stemming::StemmingTokenizer;

use crate::common::{Buildable, Configurable};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TokenizerError {
    #[error("feature '{0}' is not enabled")]
    FeatureNotEnabled(String),
    #[error("failed to load tokenizer: {0}")]
    LoadFailed(String),
}

pub trait Tokenizer: Send + Sync + Configurable<Config = TokenizerConfig> {
    fn tokenize(&self, text: &str) -> Vec<String>;
}

impl Tokenizer for Box<dyn Tokenizer> {
    fn tokenize(&self, text: &str) -> Vec<String> {
        (**self).tokenize(text)
    }
}

impl Configurable for Box<dyn Tokenizer> {
    type Config = TokenizerConfig;

    fn config(&self) -> Self::Config {
        (**self).config()
    }
}

impl Serialize for Box<dyn Tokenizer> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.config().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Box<dyn Tokenizer> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let config = TokenizerConfig::deserialize(deserializer)?;
        config.build().map_err(serde::de::Error::custom)
    }
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
}

impl Buildable for TokenizerConfig {
    type Target = Box<dyn Tokenizer>;
    type Error = TokenizerError;

    fn build(&self) -> Result<Self::Target, Self::Error> {
        match self {
            TokenizerConfig::Naive => Ok(Box::new(NaiveTokenizer::new())),
            #[cfg(feature = "hf-tokenizer")]
            TokenizerConfig::HuggingFace { model_id } => {
                let tokenizer = HfTokenizer::new(model_id)?;
                Ok(Box::new(tokenizer))
            }
            #[cfg(not(feature = "hf-tokenizer"))]
            TokenizerConfig::HuggingFace { .. } => Err(TokenizerError::FeatureNotEnabled(
                "hf-tokenizer".to_string(),
            )),
            #[cfg(feature = "stemming")]
            TokenizerConfig::Stemming => {
                let tokenizer = StemmingTokenizer::new();
                Ok(Box::new(tokenizer))
            }
            #[cfg(not(feature = "stemming"))]
            TokenizerConfig::Stemming => {
                Err(TokenizerError::FeatureNotEnabled("stemming".to_string()))
            }
        }
    }
}
