use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum NoteHandlerError {
    #[error("invalid frontmatter YAML: {0}")]
    InvalidFrontmatter(String),

    #[error("invalid tag `{tag}`: must contain only alphanumeric characters, underscores, hyphens, or slashes, and cannot be purely numeric")]
    InvalidTag { tag: String },

    #[error("not a recognized link syntax")]
    InvalidLink,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_invalid_frontmatter() {
        let err = NoteHandlerError::InvalidFrontmatter("parse error".to_string());
        let msg = err.to_string();
        assert!(msg.contains("invalid frontmatter YAML"));
        assert!(msg.contains("parse error"));
    }

    #[test]
    fn display_invalid_tag() {
        let err = NoteHandlerError::InvalidTag {
            tag: "#123".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("#123"));
    }

    #[test]
    fn display_invalid_link() {
        let err = NoteHandlerError::InvalidLink;
        assert_eq!(err.to_string(), "not a recognized link syntax");
    }
}
