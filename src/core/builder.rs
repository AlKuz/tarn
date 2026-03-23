use std::sync::Arc;

use thiserror::Error;

use crate::core::config::{Config, ConfigError, LocalStorageConfig, StorageConfig};
use crate::index::in_memory::InMemoryIndexError;
use crate::index::{InMemoryIndex, IndexConfig};
use crate::storage::local::LocalStorage;

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

    pub fn local(path: std::path::PathBuf) -> Self {
        let config = Config {
            storage: StorageConfig::Local(LocalStorageConfig { path }),
        };
        Self::from_config(config)
    }

    /// Configure the index.
    pub fn with_index(mut self, config: IndexConfig) -> Self {
        self.index_config = Some(config);
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
        let index: Option<Arc<InMemoryIndex>> = match self.index_config {
            None => None,
            Some(config) => {
                let index = InMemoryIndex::new(config).await?;
                Some(Arc::new(index))
            }
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
    pub(crate) vault_path: std::path::PathBuf,
    pub(crate) index: Option<Arc<InMemoryIndex>>,
}
