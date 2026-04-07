use thiserror::Error;

#[derive(Debug, Error)]
pub enum TokenizerError {
    #[error("feature '{0}' is not enabled")]
    FeatureNotEnabled(String),
    #[error("failed to load tokenizer: {0}")]
    LoadFailed(String),
    #[error("n-gram size must be positive, got {0}")]
    InvalidNgramSize(usize),
}
