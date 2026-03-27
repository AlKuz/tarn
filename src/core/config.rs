use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::common::Buildable;
use crate::index::in_memory::InMemoryIndexError;
use crate::index::{IndexBuildError, IndexConfig};
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index: Option<IndexConfig>,
}

impl TarnConfig {
    /// Create a config for a local vault at the given path.
    pub fn local(path: std::path::PathBuf) -> Self {
        TarnConfig {
            storage: StorageConfig::Local(LocalStorageConfig { path }),
            index: None,
        }
    }

    /// Build config from environment variables.
    pub fn from_env() -> Result<Self, ConfigError> {
        let variables: HashMap<String, String> = std::env::vars().collect();
        let storage = Self::get_storage_config(&variables)?;
        Ok(TarnConfig {
            storage,
            index: None,
        })
    }

    /// Add index configuration.
    pub fn with_index(mut self, config: IndexConfig) -> Self {
        self.index = Some(config);
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

        let index = match &self.index {
            Some(config) => {
                let idx = config.build()?;
                Some(Arc::new(idx))
            }
            None => None,
        };

        Ok(TarnCore {
            storage,
            vault_path,
            index,
        })
    }
}
