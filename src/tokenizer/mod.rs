//! Tokenizer module for text processing.
//!
//! Provides a trait-based tokenizer abstraction with feature-gated implementations:
//! - `NaiveTokenizer` — always available, whitespace-based
//! - `HfTokenizer` — HuggingFace tokenizers (requires `hf-tokenizer` feature)
//! - `StemmingTokenizer` — language-aware stemming (requires `stemming` feature)

pub mod config;
#[cfg(feature = "hf-tokenizer")]
mod hf;
mod naive;
pub mod ngram;
#[cfg(feature = "stemming")]
mod stemming;

pub use config::{TokenizerConfig, TokenizerError};
#[cfg(feature = "hf-tokenizer")]
pub use hf::HfTokenizer;
pub use naive::NaiveTokenizer;
pub use ngram::{NgramError, NgramTokenizer, NgramTokenizerConfig};
#[cfg(feature = "stemming")]
pub use stemming::StemmingTokenizer;

use crate::common::{Buildable, Configurable};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

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
