mod data_uri;

pub use data_uri::DataURI;

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
