mod data_uri;
mod revision_token;
mod vault_path;

pub use data_uri::DataURI;
pub use revision_token::RevisionToken;
use serde::{Serialize, de::DeserializeOwned};
pub use vault_path::{PathKind, VaultPath, VaultPathError};

pub trait Configurable {
    type Config: Serialize + DeserializeOwned;

    fn config(&self) -> Self::Config;
}

pub trait Buildable: Serialize + DeserializeOwned {
    type Target: Sized;
    type Error;

    fn build(&self) -> Result<Self::Target, Self::Error>;
}
