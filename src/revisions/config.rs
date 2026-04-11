use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::common::Buildable;
use crate::revisions::InMemoryRevisionTracker;
use crate::revisions::errors::RevisionTrackerError;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InMemoryRevisionTrackerConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub persistence_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RevisionTrackerConfig {
    InMemory(InMemoryRevisionTrackerConfig),
}

impl Default for RevisionTrackerConfig {
    fn default() -> Self {
        RevisionTrackerConfig::InMemory(InMemoryRevisionTrackerConfig::default())
    }
}

impl Buildable for InMemoryRevisionTrackerConfig {
    type Target = InMemoryRevisionTracker;
    type Error = RevisionTrackerError;

    fn build(&self) -> Result<Self::Target, Self::Error> {
        InMemoryRevisionTracker::new(self.persistence_path.clone())
    }
}

impl Buildable for RevisionTrackerConfig {
    type Target = InMemoryRevisionTracker;
    type Error = RevisionTrackerError;

    fn build(&self) -> Result<Self::Target, Self::Error> {
        match self {
            RevisionTrackerConfig::InMemory(config) => config.build(),
        }
    }
}
