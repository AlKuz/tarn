mod data_uri;
mod vault_path;

pub use data_uri::DataURI;
pub use vault_path::{PathKind, VaultPath, VaultPathError};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RevisionToken(String);

impl<T: Into<String>> From<T> for RevisionToken {
    fn from(value: T) -> Self {
        Self(value.into())
    }
}

impl std::fmt::Display for RevisionToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
