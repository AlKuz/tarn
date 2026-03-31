use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::common::Buildable;
use crate::storage::local::LocalStorage;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StorageConfig {
    Local(LocalStorageConfig),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalStorageConfig {
    pub path: PathBuf,
}

impl Buildable for StorageConfig {
    type Target = LocalStorage;
    type Error = std::io::Error;

    fn build(&self) -> Result<Self::Target, Self::Error> {
        match self {
            StorageConfig::Local(conf) => LocalStorage::new(conf.path.clone()),
        }
    }
}
