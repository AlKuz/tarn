mod data_uri;
mod revision_token;
mod vault_path;

use std::path::Path;

pub use data_uri::DataURI;
pub use revision_token::RevisionToken;
use serde::{Serialize, de::DeserializeOwned};
pub use vault_path::{VaultPath, VaultPathError};

pub trait Configurable {
    type Config: Serialize + DeserializeOwned;

    fn config(&self) -> Self::Config;
}

pub trait Buildable: Serialize + DeserializeOwned {
    type Target: Sized;
    type Error;

    fn build(&self) -> Result<Self::Target, Self::Error>;
}

/// Trait for components that can save/load their state to/from disk.
///
/// Each implementor is responsible for its own serialization format
/// and version management.
pub trait Persistable: Sized {
    type Error;

    /// Save state to the given file path.
    fn save(&self, path: &Path) -> Result<(), Self::Error>;

    /// Load and restore state from the given file path.
    ///
    /// Returns `Ok(true)` if state was loaded successfully.
    /// Returns `Ok(false)` if the file does not exist or the version is incompatible.
    /// Returns `Err` only for genuine I/O or deserialization failures.
    fn load(&mut self, path: &Path) -> Result<bool, Self::Error>;
}
