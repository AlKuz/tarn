use std::path::PathBuf;
use std::sync::Arc;

use thiserror::Error;

use crate::core::config::{Config, ConfigError, LocalStorageConfig, StorageConfig};
use crate::index::InMemoryIndex;
use crate::index::in_memory::InMemoryIndexError;
use crate::storage::local::LocalStorage;

/// Configuration for the index.
#[derive(Clone)]
pub enum IndexConfig {
    /// Ephemeral in-memory index (lost on restart).
    InMemory { tokenizer_id: String },
    /// Persistent in-memory index (saved to disk).
    Persistent { tokenizer_id: String, path: PathBuf },
}

/// Errors that can occur during TarnCore build.
#[derive(Debug, Error)]
pub enum BuildError {
    #[error("index initialization failed: {0}")]
    Index(#[from] InMemoryIndexError),
}

pub struct TarnBuilder {
    config: Config,
    index_config: Option<IndexConfig>,
}

impl TarnBuilder {
    pub fn from_config(config: Config) -> Self {
        TarnBuilder {
            config,
            index_config: None,
        }
    }

    pub fn from_env() -> Result<Self, ConfigError> {
        let config = Config::from_env()?;
        Ok(Self::from_config(config))
    }

    pub fn local(path: PathBuf) -> Self {
        let config = Config {
            storage: StorageConfig::Local(LocalStorageConfig { path }),
        };
        Self::from_config(config)
    }

    /// Configure an ephemeral in-memory index.
    pub fn with_index(mut self, tokenizer_id: &str) -> Self {
        self.index_config = Some(IndexConfig::InMemory {
            tokenizer_id: tokenizer_id.to_string(),
        });
        self
    }

    /// Configure a persistent in-memory index.
    pub fn with_persistent_index(mut self, tokenizer_id: &str, path: PathBuf) -> Self {
        self.index_config = Some(IndexConfig::Persistent {
            tokenizer_id: tokenizer_id.to_string(),
            path,
        });
        self
    }

    /// Build TarnCore without an index.
    pub fn build(self) -> TarnCore {
        match self.config.storage {
            StorageConfig::Local(conf) => TarnCore {
                vault_path: conf.path.clone(),
                storage: LocalStorage::new(conf.path),
                index: None,
            },
        }
    }

    /// Build TarnCore with async index initialization.
    pub async fn build_async(self) -> Result<TarnCore, BuildError> {
        let index: Option<Arc<InMemoryIndex>> = match &self.index_config {
            None => None,
            Some(IndexConfig::InMemory { tokenizer_id }) => {
                Some(Arc::new(InMemoryIndex::new(tokenizer_id)?))
            }
            Some(IndexConfig::Persistent { tokenizer_id, path }) => Some(Arc::new(
                InMemoryIndex::with_persistence(path, tokenizer_id).await?,
            )),
        };

        match self.config.storage {
            StorageConfig::Local(conf) => Ok(TarnCore {
                vault_path: conf.path.clone(),
                storage: LocalStorage::new(conf.path),
                index,
            }),
        }
    }
}

pub struct TarnCore {
    pub(crate) storage: LocalStorage,
    pub(crate) vault_path: PathBuf,
    pub(crate) index: Option<Arc<InMemoryIndex>>,
}
