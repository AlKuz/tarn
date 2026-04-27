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
use crate::revisions::{
    InMemoryRevisionTrackerConfig, RevisionTracker, RevisionTrackerConfig, RevisionTrackerError,
};
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
    #[error("revision tracker initialization failed: {0}")]
    Revisions(#[from] RevisionTrackerError),
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
pub struct TarnConfig<
    SC = StorageConfig,
    IC = IndexConfig,
    OC = ObserverConfig,
    RC = RevisionTrackerConfig,
> {
    pub vault_name: String,
    pub storage: SC,
    pub index: IC,
    pub observer: OC,
    pub revisions: RC,
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
                persistence_path: persistence_path.clone(),
                ..Default::default()
            }),
            observer,
            revisions: RevisionTrackerConfig::InMemory(InMemoryRevisionTrackerConfig {
                persistence_path,
            }),
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
                persistence_path: persistence_path.clone(),
                ..Default::default()
            }),
            observer,
            revisions: RevisionTrackerConfig::InMemory(InMemoryRevisionTrackerConfig {
                persistence_path,
            }),
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

impl<SC, IC, OC, RC> Buildable for TarnConfig<SC, IC, OC, RC>
where
    SC: Buildable + Serialize + for<'de> Deserialize<'de>,
    IC: Buildable + Serialize + for<'de> Deserialize<'de>,
    OC: Buildable + Serialize + for<'de> Deserialize<'de>,
    RC: Buildable + Serialize + for<'de> Deserialize<'de>,
    SC::Target: Storage + Send + Sync + 'static,
    IC::Target: Index + Send + Sync + 'static,
    OC::Target: Observer + Send + Sync + 'static,
    RC::Target: RevisionTracker + Send + Sync + 'static,
    BuildError: From<SC::Error> + From<IC::Error> + From<OC::Error> + From<RC::Error>,
{
    type Target = TarnCore<SC::Target, IC::Target, OC::Target, RC::Target>;
    type Error = BuildError;

    fn build(&self) -> Result<Self::Target, Self::Error> {
        let storage = Arc::new(self.storage.build()?);
        let index = Arc::new(self.index.build()?);
        let observer = Arc::new(self.observer.build()?);
        let revisions = Arc::new(self.revisions.build()?);

        Ok(TarnCore::new(
            storage,
            self.vault_name.clone(),
            index,
            observer,
            revisions,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_storage_config_local() {
        let mut vars = HashMap::new();
        vars.insert("STORAGE__TYPE".into(), "local".into());
        vars.insert("STORAGE__PATH".into(), "/tmp/vault".into());

        let config = TarnConfig::get_storage_config(&vars).unwrap();
        match config {
            StorageConfig::Local(c) => assert_eq!(c.path, std::path::PathBuf::from("/tmp/vault")),
        }
    }

    #[test]
    fn get_storage_config_missing_type() {
        let vars = HashMap::new();
        let err = TarnConfig::get_storage_config(&vars).unwrap_err();
        assert!(matches!(err, ConfigError::MissingVar(_)));
    }

    #[test]
    fn get_storage_config_unsupported_type() {
        let mut vars = HashMap::new();
        vars.insert("STORAGE__TYPE".into(), "s3".into());

        let err = TarnConfig::get_storage_config(&vars).unwrap_err();
        assert!(matches!(err, ConfigError::UnsupportedStorageType(_)));
    }

    #[test]
    fn get_storage_config_missing_path() {
        let mut vars = HashMap::new();
        vars.insert("STORAGE__TYPE".into(), "local".into());

        let err = TarnConfig::get_storage_config(&vars).unwrap_err();
        assert!(matches!(err, ConfigError::MissingVar(_)));
    }

    #[test]
    fn with_observer_overrides() {
        let config = TarnConfig::local("/tmp/vault".into());
        let new_observer = ObserverConfig::Local {
            path: "/tmp/other".into(),
        };
        let config = config.with_observer(new_observer.clone());
        match &config.observer {
            ObserverConfig::Local { path } => {
                assert_eq!(path, &std::path::PathBuf::from("/tmp/other"));
            }
        }
    }

    #[test]
    fn with_index_overrides() {
        let config = TarnConfig::local("/tmp/vault".into());
        let new_index = IndexConfig::InMemory(InMemoryIndexConfig {
            persistence_path: Some("/tmp/custom.json".into()),
            ..Default::default()
        });
        let config = config.with_index(new_index);
        match &config.index {
            IndexConfig::InMemory(c) => {
                assert_eq!(
                    c.persistence_path,
                    Some(std::path::PathBuf::from("/tmp/custom.json"))
                );
            }
        }
    }

    #[test]
    fn build_error_from_in_memory_index_error() {
        let io_err = std::io::Error::other("test");
        let index_err = InMemoryIndexError::Io(io_err);
        let build_err: BuildError = index_err.into();
        assert!(matches!(build_err, BuildError::Index(_)));
    }

    #[test]
    fn build_error_from_revision_tracker_error() {
        let io_err = std::io::Error::other("test");
        let rev_err = RevisionTrackerError::Io(io_err);
        let build_err: BuildError = rev_err.into();
        assert!(matches!(build_err, BuildError::Revisions(_)));
    }
}
