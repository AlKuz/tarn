use std::collections::HashMap;
use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("missing environment variable: {0}")]
    MissingVar(String),
    #[error("unsupported storage type: {0}")]
    UnsupportedStorageType(String),
}

pub enum StorageConfig {
    Local(LocalStorageConfig),
}

pub struct LocalStorageConfig {
    pub path: PathBuf,
}

pub struct Config {
    pub storage: StorageConfig,
}

impl Config {
    fn get_storage_config(variables: &HashMap<String, String>) -> Result<StorageConfig, ConfigError> {
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

    pub fn from_env() -> Result<Self, ConfigError> {
        let variables: HashMap<String, String> = std::env::vars().collect();
        Ok(Config {
            storage: Self::get_storage_config(&variables)?,
        })
    }
}
