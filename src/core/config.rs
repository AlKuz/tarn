use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::common::Buildable;
use crate::index::in_memory::InMemoryIndexError;
use crate::index::{IndexBuildError, IndexConfig, default_persistence_path};
use crate::storage::{LocalStorageConfig, StorageConfig};

use super::tarn_core::TarnCore;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("missing environment variable: {0}")]
    MissingVar(String),
    #[error("unsupported storage type: {0}")]
    UnsupportedStorageType(String),
}

#[derive(Debug, Error)]
pub enum BuildError {
    #[error("index initialization failed: {0}")]
    Index(#[from] IndexBuildError),
    #[error("storage initialization failed: {0}")]
    Storage(#[from] std::io::Error),
}

impl From<InMemoryIndexError> for BuildError {
    fn from(e: InMemoryIndexError) -> Self {
        BuildError::Index(IndexBuildError::Index(e))
    }
}

// ---------------------------------------------------------------------------
// TarnConfig
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TarnConfig {
    pub storage: StorageConfig,
    pub index: IndexConfig,
}

impl TarnConfig {
    /// Create a config for a local vault at the given path.
    ///
    /// Creates a default in-memory index with auto-computed persistence path
    /// based on the platform data directory.
    pub fn local(path: std::path::PathBuf) -> Self {
        let persistence_path = default_persistence_path(&path);
        TarnConfig {
            storage: StorageConfig::Local(LocalStorageConfig { path }),
            index: IndexConfig::InMemory {
                tokenizer: Default::default(),
                persistence_path,
            },
        }
    }

    /// Build config from environment variables.
    pub fn from_env() -> Result<Self, ConfigError> {
        let variables: HashMap<String, String> = std::env::vars().collect();
        let storage = Self::get_storage_config(&variables)?;
        let vault_path = match &storage {
            StorageConfig::Local(conf) => &conf.path,
        };
        let persistence_path = default_persistence_path(vault_path);
        Ok(TarnConfig {
            storage,
            index: IndexConfig::InMemory {
                tokenizer: Default::default(),
                persistence_path,
            },
        })
    }

    /// Override the index configuration.
    pub fn with_index(mut self, config: IndexConfig) -> Self {
        self.index = config;
        self
    }

    fn get_storage_config(
        variables: &HashMap<String, String>,
    ) -> Result<StorageConfig, ConfigError> {
        let storage_type = variables
            .get("STORAGE__TYPE")
            .ok_or_else(|| ConfigError::MissingVar("STORAGE__TYPE".into()))?;
        match storage_type.as_str() {
            "local" => {
                let path = variables
                    .get("STORAGE__PATH")
                    .ok_or_else(|| ConfigError::MissingVar("STORAGE__PATH".into()))?
                    .into();
                Ok(StorageConfig::Local(LocalStorageConfig { path }))
            }
            _ => Err(ConfigError::UnsupportedStorageType(storage_type.clone())),
        }
    }
}

impl Buildable for TarnConfig {
    type Target = TarnCore;
    type Error = BuildError;

    fn build(&self) -> Result<Self::Target, Self::Error> {
        let storage = self.storage.build()?;
        let vault_path = match &self.storage {
            StorageConfig::Local(conf) => conf.path.clone(),
        };

        let index = Arc::new(self.index.build()?);

        Ok(TarnCore {
            storage,
            vault_path,
            index,
        })
    }
}
