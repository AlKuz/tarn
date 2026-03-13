use std::path::PathBuf;

use crate::core::config::{Config, ConfigError, LocalStorageConfig, StorageConfig};
use crate::storage::local::LocalStorage;

pub struct TarnBuilder {
    config: Config,
}

impl TarnBuilder {
    pub fn from_config(config: Config) -> Self {
        TarnBuilder { config }
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

    pub fn build(self) -> TarnCore {
        match self.config.storage {
            StorageConfig::Local(conf) => TarnCore {
                vault_path: conf.path.clone(),
                storage: LocalStorage::new(conf.path),
            },
        }
    }
}

pub struct TarnCore {
    pub(crate) storage: LocalStorage,
    pub(crate) vault_path: PathBuf,
}
