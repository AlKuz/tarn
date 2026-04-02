use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::common::Buildable;
use crate::index::in_memory::InMemoryIndexError;
use crate::index::{
    InMemoryIndexConfig, Index, IndexBuildError, IndexConfig, default_persistence_path,
};
use crate::observer::{Observer, ObserverConfig};
use crate::storage::{LocalStorageConfig, Storage, StorageConfig};

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

impl From<std::convert::Infallible> for BuildError {
    fn from(e: std::convert::Infallible) -> Self {
        match e {}
    }
}

// ---------------------------------------------------------------------------
// TarnConfig
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TarnConfig<SC = StorageConfig, IC = IndexConfig, OC = ObserverConfig> {
    pub vault_name: String,
    pub storage: SC,
    pub index: IC,
    pub observer: OC,
}

impl TarnConfig {
    /// Create a config for a local vault at the given path.
    ///
    /// Creates a default in-memory index with auto-computed persistence path
    /// based on the platform data directory.
    pub fn local(path: std::path::PathBuf) -> Self {
        let persistence_path = default_persistence_path(&path);
        let vault_name = path.to_string_lossy().to_string();
        let observer = ObserverConfig::Local { path: path.clone() };
        TarnConfig {
            vault_name,
            storage: StorageConfig::Local(LocalStorageConfig { path }),
            index: IndexConfig::InMemory(InMemoryIndexConfig {
                persistence_path,
                ..Default::default()
            }),
            observer,
        }
    }

    /// Build config from environment variables.
    pub fn from_env() -> Result<Self, ConfigError> {
        let variables: HashMap<String, String> = std::env::vars().collect();
        let storage = Self::get_storage_config(&variables)?;
        let vault_path = match &storage {
            StorageConfig::Local(conf) => &conf.path,
        };
        let vault_name = vault_path.to_string_lossy().to_string();
        let persistence_path = default_persistence_path(vault_path);
        let observer = ObserverConfig::Local {
            path: vault_path.clone(),
        };
        Ok(TarnConfig {
            vault_name,
            storage,
            index: IndexConfig::InMemory(InMemoryIndexConfig {
                persistence_path,
                ..Default::default()
            }),
            observer,
        })
    }

    /// Override the index configuration.
    pub fn with_index(mut self, config: IndexConfig) -> Self {
        self.index = config;
        self
    }

    /// Override the observer configuration.
    pub fn with_observer(mut self, config: ObserverConfig) -> Self {
        self.observer = config;
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

impl<SC, IC, OC> Buildable for TarnConfig<SC, IC, OC>
where
    SC: Buildable + Serialize + for<'de> Deserialize<'de>,
    IC: Buildable + Serialize + for<'de> Deserialize<'de>,
    OC: Buildable + Serialize + for<'de> Deserialize<'de>,
    SC::Target: Storage + Send + Sync + 'static,
    IC::Target: Index + Send + Sync + 'static,
    OC::Target: Observer + Send + Sync + 'static,
    BuildError: From<SC::Error> + From<IC::Error> + From<OC::Error>,
{
    type Target = TarnCore<SC::Target, IC::Target, OC::Target>;
    type Error = BuildError;

    fn build(&self) -> Result<Self::Target, Self::Error> {
        let storage = Arc::new(self.storage.build()?);
        let index = Arc::new(self.index.build()?);
        let observer = Arc::new(self.observer.build()?);

        Ok(TarnCore::new(
            storage,
            self.vault_name.clone(),
            index,
            observer,
        ))
    }
}
