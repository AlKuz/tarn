use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::common::Buildable;
use crate::index::InMemoryIndex;
use crate::index::in_memory::InMemoryIndexError;
use crate::index::in_memory::{BM25Config, RRFConfig, TagIndexConfig};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InMemoryIndexConfig {
    #[serde(default)]
    pub bm25_index: BM25Config,
    #[serde(default)]
    pub tag_index: TagIndexConfig,
    #[serde(default)]
    pub rrf: RRFConfig,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub persistence_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IndexConfig {
    InMemory(InMemoryIndexConfig),
}

/// Errors that can occur when building an index from config.
#[derive(Debug, Error)]
pub enum IndexBuildError {
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
pub fn default_persistence_path(vault_path: &std::path::Path) -> Option<PathBuf> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let data_dir = dirs::data_local_dir()?;
    let mut hasher = DefaultHasher::new();
    vault_path.hash(&mut hasher);
    let hash = format!("{:016x}", hasher.finish());
    Some(data_dir.join("tarn").join(hash).join("index.json"))
}

impl Buildable for InMemoryIndexConfig {
    type Target = InMemoryIndex;
    type Error = IndexBuildError;

    fn build(&self) -> Result<Self::Target, Self::Error> {
        let bm25_index = self.bm25_index.build().unwrap();
        let tag_idx = self.tag_index.build().unwrap();
        let rrf_instance = self.rrf.build().unwrap();
        let index = InMemoryIndex::new(
            bm25_index,
            tag_idx,
            rrf_instance,
            self.persistence_path.clone(),
        )?;
        Ok(index)
    }
}

impl Buildable for IndexConfig {
    type Target = InMemoryIndex;
    type Error = IndexBuildError;

    fn build(&self) -> Result<Self::Target, Self::Error> {
        match self {
            IndexConfig::InMemory(config) => config.build(),
        }
    }
}
