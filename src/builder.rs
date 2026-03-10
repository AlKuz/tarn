use crate::config::{Config, StorageConfig};
use crate::storage::local::LocalStorage;
use crate::storage::Storage;

struct Builder {
    config: Config,
}

impl Builder {
    pub fn new(config: Config) -> Self {
        Builder { config }
    }

    pub fn get_storage(&self) -> impl Storage {
        match &self.config.storage {
            StorageConfig::Local(conf) => LocalStorage::new(conf.path.clone())
        }
    }
}