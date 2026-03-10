use std::fmt;
use std::str::FromStr;
use base64::Engine;

#[derive(Debug, thiserror::Error)]
pub enum DataURIError {
    #[error("missing 'data:' prefix")]
    MissingPrefix,
    #[error("missing ';base64,' separator")]
    MissingSeparator,
    #[error("empty MIME type")]
    EmptyMime,
    #[error("invalid base64 data: {0}")]
    InvalidBase64(#[from] base64::DecodeError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataURI {
    mime: String,
    data: String,
}

impl DataURI {
    pub fn new(mime: String, data: &[u8]) -> Self {
        DataURI {
            mime,
            data: base64::engine::general_purpose::STANDARD.encode(data),
        }
    }
    
    pub fn decode(&self) -> Result<Vec<u8>, DataURIError> {
        base64::engine::general_purpose::STANDARD.decode(&self.data).map_err(DataURIError::InvalidBase64)
    }
}

impl FromStr for DataURI {
    type Err = DataURIError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let rest = s.strip_prefix("data:").ok_or(DataURIError::MissingPrefix)?;
        let (mime, data) = rest.split_once(";base64,").ok_or(DataURIError::MissingSeparator)?;
        if mime.is_empty() {
            return Err(DataURIError::EmptyMime);
        }
        base64::engine::general_purpose::STANDARD.decode(data)?;
        Ok(DataURI {
            mime: mime.to_string(),
            data: data.to_string(),
        })
    }
}

impl fmt::Display for DataURI {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "data:{};base64,{}", self.mime, self.data)
    }
}