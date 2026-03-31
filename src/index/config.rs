use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::common::Buildable;
use crate::index::InMemoryIndex;
use crate::index::in_memory::InMemoryIndexError;
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

/// Compute the default persistence path for an index.
///
/// Uses the platform-specific data directory (via `dirs::data_local_dir()`)
/// with a hash of the vault path to distinguish multiple vaults:
/// - Linux: `~/.local/share/tarn/<vault-hash>/index.json`
/// - macOS: `~/Library/Application Support/tarn/<vault-hash>/index.json`
/// - Windows: `C:\Users\<user>\AppData\Local\tarn\<vault-hash>\index.json`
///
/// Returns `None` if the platform data directory cannot be determined.
pub fn default_persistence_path(vault_path: &Path) -> Option<PathBuf> {
    let data_dir = dirs::data_local_dir()?;
    let mut hasher = DefaultHasher::new();
    vault_path.hash(&mut hasher);
    let hash = format!("{:016x}", hasher.finish());
    Some(data_dir.join("tarn").join(hash).join("index.json"))
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
