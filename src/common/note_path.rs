use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum NotePathError {
    #[error("note path must have .md extension")]
    InvalidExtension,
    #[error("note path cannot be empty")]
    Empty,
    #[error("note path cannot contain '..' traversal")]
    PathTraversal,
}

/// A validated, platform-independent note path.
///
/// Internally stores the path with forward slashes (`/`) as separators,
/// regardless of the platform. This ensures consistent behavior for shared
/// indexes across Unix and Windows systems.
///
/// # Examples
///
/// ```
/// use tarn::common::NotePath;
///
/// // Parse from string (accepts both / and \ separators)
/// let path: NotePath = "projects/alpha/design.md".parse().unwrap();
/// assert_eq!(path.as_str(), "projects/alpha/design.md");
///
/// // Convert to native filesystem path
/// let fs_path = path.as_path_buf();
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct NotePath(String);

impl NotePath {
    /// Creates a new NotePath, validating the extension and normalizing separators.
    pub fn new(path: impl AsRef<str>) -> Result<Self, NotePathError> {
        let path = path.as_ref();

        if path.is_empty() {
            return Err(NotePathError::Empty);
        }

        // Normalize separators to forward slashes
        let normalized = normalize_separators(path);

        // Check for path traversal
        if normalized.split('/').any(|segment| segment == "..") {
            return Err(NotePathError::PathTraversal);
        }

        // Validate .md extension
        if !normalized.ends_with(".md") {
            return Err(NotePathError::InvalidExtension);
        }

        Ok(NotePath(normalized))
    }

    /// Returns the normalized path as a string slice (always uses `/` separators).
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Converts to a platform-native `PathBuf`.
    pub fn as_path_buf(&self) -> PathBuf {
        PathBuf::from(&self.0)
    }

    /// Creates a NotePath from a filesystem Path, making it relative to the given root.
    ///
    /// Returns `None` if the path is not under root or doesn't have .md extension.
    pub fn from_path(path: &Path, root: &Path) -> Result<Self, NotePathError> {
        let relative = path
            .strip_prefix(root)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| path.to_string_lossy().to_string());

        NotePath::new(relative)
    }

    /// Returns the note title derived from the filename (without .md extension).
    pub fn stem(&self) -> &str {
        self.0
            .rsplit('/')
            .next()
            .and_then(|name| name.strip_suffix(".md"))
            .unwrap_or(&self.0)
    }

    /// Returns the parent folder path, or None if at root.
    pub fn parent(&self) -> Option<&str> {
        self.0.rsplit_once('/').map(|(parent, _)| parent)
    }

    /// Checks if this note is in the given folder (non-recursive).
    pub fn is_in_folder(&self, folder: &str) -> bool {
        let folder = folder.trim_matches('/');
        match self.parent() {
            Some(parent) => parent == folder,
            None => folder.is_empty(),
        }
    }

    /// Checks if this note is under the given folder (recursive).
    pub fn is_under_folder(&self, folder: &str) -> bool {
        let folder = folder.trim_matches('/');
        if folder.is_empty() {
            return true;
        }
        self.0.starts_with(folder)
            && self
                .0
                .as_bytes()
                .get(folder.len())
                .is_some_and(|&b| b == b'/')
    }
}

/// Normalizes path separators to forward slashes.
fn normalize_separators(path: &str) -> String {
    // Remove leading slashes and normalize backslashes
    let path = path.trim_start_matches(['/', '\\']);
    path.replace('\\', "/")
}

impl FromStr for NotePath {
    type Err = NotePathError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        NotePath::new(s)
    }
}

impl fmt::Display for NotePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for NotePath {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl AsRef<Path> for NotePath {
    fn as_ref(&self) -> &Path {
        Path::new(&self.0)
    }
}

impl From<NotePath> for String {
    fn from(path: NotePath) -> Self {
        path.0
    }
}

impl From<NotePath> for PathBuf {
    fn from(path: NotePath) -> Self {
        path.as_path_buf()
    }
}

impl TryFrom<String> for NotePath {
    type Error = NotePathError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        NotePath::new(value)
    }
}

impl TryFrom<&str> for NotePath {
    type Error = NotePathError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        NotePath::new(value)
    }
}

impl TryFrom<PathBuf> for NotePath {
    type Error = NotePathError;

    fn try_from(value: PathBuf) -> Result<Self, Self::Error> {
        NotePath::new(value.to_string_lossy())
    }
}

impl TryFrom<&Path> for NotePath {
    type Error = NotePathError;

    fn try_from(value: &Path) -> Result<Self, Self::Error> {
        NotePath::new(value.to_string_lossy())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_simple_path() {
        let path = NotePath::new("note.md").unwrap();
        assert_eq!(path.as_str(), "note.md");
        assert_eq!(path.stem(), "note");
        assert_eq!(path.parent(), None);
    }

    #[test]
    fn valid_nested_path() {
        let path = NotePath::new("projects/alpha/design.md").unwrap();
        assert_eq!(path.as_str(), "projects/alpha/design.md");
        assert_eq!(path.stem(), "design");
        assert_eq!(path.parent(), Some("projects/alpha"));
    }

    #[test]
    fn normalizes_backslashes() {
        let path = NotePath::new(r"projects\alpha\design.md").unwrap();
        assert_eq!(path.as_str(), "projects/alpha/design.md");
    }

    #[test]
    fn normalizes_mixed_separators() {
        let path = NotePath::new(r"projects/alpha\beta/note.md").unwrap();
        assert_eq!(path.as_str(), "projects/alpha/beta/note.md");
    }

    #[test]
    fn strips_leading_slash() {
        let path = NotePath::new("/projects/note.md").unwrap();
        assert_eq!(path.as_str(), "projects/note.md");
    }

    #[test]
    fn strips_leading_backslash() {
        let path = NotePath::new(r"\projects\note.md").unwrap();
        assert_eq!(path.as_str(), "projects/note.md");
    }

    #[test]
    fn rejects_non_md_extension() {
        let err = NotePath::new("note.txt").unwrap_err();
        assert_eq!(err, NotePathError::InvalidExtension);
    }

    #[test]
    fn rejects_no_extension() {
        let err = NotePath::new("note").unwrap_err();
        assert_eq!(err, NotePathError::InvalidExtension);
    }

    #[test]
    fn rejects_empty_path() {
        let err = NotePath::new("").unwrap_err();
        assert_eq!(err, NotePathError::Empty);
    }

    #[test]
    fn rejects_path_traversal() {
        let err = NotePath::new("../secret/note.md").unwrap_err();
        assert_eq!(err, NotePathError::PathTraversal);

        let err = NotePath::new("projects/../note.md").unwrap_err();
        assert_eq!(err, NotePathError::PathTraversal);
    }

    #[test]
    fn is_in_folder_root() {
        let path = NotePath::new("note.md").unwrap();
        assert!(path.is_in_folder(""));
        assert!(path.is_in_folder("/"));
        assert!(!path.is_in_folder("projects"));
    }

    #[test]
    fn is_in_folder_nested() {
        let path = NotePath::new("projects/alpha/note.md").unwrap();
        assert!(path.is_in_folder("projects/alpha"));
        assert!(path.is_in_folder("/projects/alpha/"));
        assert!(!path.is_in_folder("projects"));
        assert!(!path.is_in_folder(""));
    }

    #[test]
    fn is_under_folder() {
        let path = NotePath::new("projects/alpha/note.md").unwrap();
        assert!(path.is_under_folder(""));
        assert!(path.is_under_folder("projects"));
        assert!(path.is_under_folder("projects/alpha"));
        assert!(!path.is_under_folder("projects/beta"));
        assert!(!path.is_under_folder("other"));
    }

    #[test]
    fn is_under_folder_no_partial_match() {
        let path = NotePath::new("projects-old/note.md").unwrap();
        assert!(!path.is_under_folder("projects"));
    }

    #[test]
    fn serde_roundtrip() {
        let path = NotePath::new("projects/note.md").unwrap();
        let json = serde_json::to_string(&path).unwrap();
        assert_eq!(json, "\"projects/note.md\"");

        let parsed: NotePath = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, path);
    }

    #[test]
    fn serde_rejects_invalid() {
        let result: Result<NotePath, _> = serde_json::from_str("\"note.txt\"");
        assert!(result.is_err());
    }

    #[test]
    fn from_path_relative() {
        let root = Path::new("/vault");
        let path = Path::new("/vault/projects/note.md");
        let note_path = NotePath::from_path(path, root).unwrap();
        assert_eq!(note_path.as_str(), "projects/note.md");
    }

    #[test]
    fn display_and_to_string() {
        let path = NotePath::new("projects/note.md").unwrap();
        assert_eq!(path.to_string(), "projects/note.md");
        assert_eq!(format!("{path}"), "projects/note.md");
    }

    #[test]
    fn to_path_buf() {
        let path = NotePath::new("projects/note.md").unwrap();
        let pb = path.as_path_buf();
        assert_eq!(pb, PathBuf::from("projects/note.md"));
    }

    #[test]
    fn parse_from_str() {
        let path: NotePath = "daily/2024-03-14.md".parse().unwrap();
        assert_eq!(path.as_str(), "daily/2024-03-14.md");
    }

    #[test]
    fn try_from_implementations() {
        let from_string: NotePath = String::from("note.md").try_into().unwrap();
        assert_eq!(from_string.as_str(), "note.md");

        let from_str: NotePath = "note.md".try_into().unwrap();
        assert_eq!(from_str.as_str(), "note.md");

        let from_pathbuf: NotePath = PathBuf::from("note.md").try_into().unwrap();
        assert_eq!(from_pathbuf.as_str(), "note.md");
    }
}
