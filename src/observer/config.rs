use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::common::Buildable;
use crate::observer::local::LocalStorageObserver;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ObserverConfig {
    Local { path: PathBuf },
}

impl Buildable for ObserverConfig {
    type Target = LocalStorageObserver;
    type Error = std::convert::Infallible;

    fn build(&self) -> Result<Self::Target, Self::Error> {
        match self {
            ObserverConfig::Local { path } => Ok(LocalStorageObserver::new(path.clone())),
        }
    }
}
