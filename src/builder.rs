use crate::config::{Config, StorageConfig};
use crate::storage::Storage;
use crate::storage::local::LocalStorage;

struct Builder {
    config: Config,
}

impl Builder {
    pub fn new(config: Config) -> Self {
        Builder { config }
    }

    pub fn get_storage(&self) -> impl Storage {
        match &self.config.storage {
            StorageConfig::Local(conf) => LocalStorage::new(conf.path.clone()),
        }
    }
}
