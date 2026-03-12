use crate::core::config::{Config, StorageConfig};
use crate::core::storage::Storage;
use crate::core::storage::local::LocalStorage;

pub(crate) struct Builder {
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
