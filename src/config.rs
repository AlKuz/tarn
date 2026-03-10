use std::collections::HashMap;
use std::path::PathBuf;

pub enum StorageConfig{
    Local(LocalStorageConfig),
}

pub struct LocalStorageConfig{
    pub path: PathBuf,
}


pub struct Config{
    pub storage: StorageConfig
}

impl Config {
    fn get_storage_config(variables: &HashMap<String, String>) -> StorageConfig {
        let storage_type = variables
            .get("STORAGE__TYPE")
            .expect("STORAGE__TYPE missing in environment variables");
        match storage_type.as_str() {
            "local" => {
                let path = variables
                    .get("STORAGE__PATH")
                    .expect("STORAGE__PATH missing for local storage")
                    .into();
                StorageConfig::Local(LocalStorageConfig { path })
            },
            _ => panic!("Unsupported storage type: {}", storage_type),
        }
    }

    pub fn from_env() -> Self {
        let variables: HashMap<String, String> = std::env::vars().collect();
        Config {
            storage: Self::get_storage_config(&variables),
        }
    }
}