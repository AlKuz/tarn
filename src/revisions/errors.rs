use thiserror::Error;

/// Errors specific to revision tracker operations.
#[derive(Debug, Error)]
pub enum RevisionTrackerError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}
