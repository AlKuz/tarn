use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::common::Buildable;
use crate::index::in_memory::InMemoryIndexError;
use crate::index::InMemoryIndex;
use crate::tokenizer::TokenizerConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IndexConfig {
    InMemory {
        #[serde(default)]
        tokenizer: TokenizerConfig,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        persistence_path: Option<PathBuf>,
    },
}

/// Errors that can occur when building an index from config.
#[derive(Debug, Error)]
pub enum IndexBuildError {
    #[error(transparent)]
    Tokenizer(#[from] crate::tokenizer::TokenizerError),
    #[error(transparent)]
    Index(#[from] InMemoryIndexError),
}

impl Buildable for IndexConfig {
    type Target = InMemoryIndex;
    type Error = IndexBuildError;

    fn build(&self) -> Result<Self::Target, Self::Error> {
        match self {
            IndexConfig::InMemory {
                tokenizer,
                persistence_path,
            } => {
                let tok = tokenizer.clone().build()?;
                let index = InMemoryIndex::new(tok, persistence_path.clone())?;
                Ok(index)
            }
        }
    }
}
