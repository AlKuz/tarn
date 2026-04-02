//! Tokenizer module providing trait-based text tokenization.
//!
//! ## Structure
//!
//! - `mod.rs` — `Tokenizer` trait, `Box<dyn Tokenizer>` support, re-exports
//! - `config.rs` — `TokenizerConfig` dispatch enum, `Buildable` impl
//! - `errors.rs` — `TokenizerError` unified error type
//! - `naive.rs` — `NaiveTokenizer` (whitespace-based, always available)
//! - `ngram.rs` — `NgramTokenizer` (character n-grams for trigram similarity)
//! - `hf.rs` — `HfTokenizer` (HuggingFace subword tokenizers, feature-gated)
//! - `stemming.rs` — `StemmingTokenizer` (language-aware stemming, feature-gated)

mod config;
mod errors;
#[cfg(feature = "hf-tokenizer")]
mod hf;
mod naive;
mod ngram;
#[cfg(feature = "stemming")]
mod stemming;

pub use config::TokenizerConfig;
pub use errors::TokenizerError;
#[cfg(feature = "hf-tokenizer")]
pub use hf::HfTokenizer;
pub use naive::NaiveTokenizer;
pub use ngram::NgramTokenizer;
#[cfg(feature = "stemming")]
pub use stemming::StemmingTokenizer;

use crate::common::Configurable;

pub trait Tokenizer: Send + Sync + Configurable<Config = TokenizerConfig> {
    fn tokenize(&self, text: &str) -> Vec<String>;
}
