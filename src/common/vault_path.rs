use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum VaultPathError {
    #[error("vault path cannot be empty")]
    Empty,
    #[error("vault path cannot contain '..' traversal")]
    PathTraversal,
    #[error("path is not under root")]
    NotUnderRoot,
}

/// A validated, platform-independent vault path.
///
/// Internally stores the path with forward slashes (`/`) as separators,
/// regardless of the platform. This ensures consistent behavior for shared
/// indexes across Unix and Windows systems.
///
/// The path variant is auto-detected from the file extension:
/// - `.md` -> `VaultPath::Note`
/// - `.png`, `.jpg`, `.jpeg`, `.gif`, `.bmp`, `.webp`, `.svg`, `.ico`, `.tiff`, `.tif` -> `VaultPath::Image`
/// - anything else -> `VaultPath::Any`
///
/// # Examples
///
/// ```
/// use tarn::common::VaultPath;
///
/// // Parse from string (auto-detects variant from extension)
/// let note: VaultPath = "projects/alpha/design.md".try_into().unwrap();
/// assert!(matches!(note, VaultPath::Note(_)));
///
/// let img: VaultPath = "assets/logo.png".try_into().unwrap();
/// assert!(matches!(img, VaultPath::Image(_)));
///
/// let other: VaultPath = "data.json".try_into().unwrap();
/// assert!(matches!(other, VaultPath::Any(_)));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum VaultPath {
    /// Markdown note (.md extension)
    Note(String),
    /// Image file (png, jpg, gif, etc.)
    Image(String),
    /// Any other file type
    Any(String),
}

impl VaultPath {
    /// Creates a new VaultPath, validating the path and normalizing separators.
    /// The variant is auto-detected from the file extension.
    pub fn new(path: impl AsRef<str>) -> Result<Self, VaultPathError> {
        let path = path.as_ref();

        if path.is_empty() {
            return Err(VaultPathError::Empty);
        }

        let normalized = normalize_separators(path);

        if normalized.split('/').any(|segment| segment == "..") {
            return Err(VaultPathError::PathTraversal);
        }

        let ext = normalized
            .rsplit('.')
            .next()
            .map(|e| e.to_lowercase())
            .unwrap_or_default();

        let variant = match ext.as_str() {
            "md" => VaultPath::Note(normalized),
            "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "svg" | "ico" | "tiff" | "tif" => {
                VaultPath::Image(normalized)
            }
            _ => VaultPath::Any(normalized),
        };

        Ok(variant)
    }

    /// Returns the normalized path as a string slice (always uses `/` separators).
    pub fn as_str(&self) -> &str {
        match self {
            VaultPath::Note(s) | VaultPath::Image(s) | VaultPath::Any(s) => s,
        }
    }

    /// Converts to a platform-native `PathBuf`.
    pub fn as_path_buf(&self) -> PathBuf {
        PathBuf::from(self.as_str())
    }

    /// Creates a VaultPath from a filesystem Path, making it relative to the given root.
    ///
    /// Returns `NotUnderRoot` error if path is not under root.
    pub fn from_path(path: &Path, root: &Path) -> Result<Self, VaultPathError> {
        let relative = path
            .strip_prefix(root)
            .map_err(|_| VaultPathError::NotUnderRoot)?
            .to_string_lossy();

        VaultPath::new(relative)
    }

    /// Returns the file stem (filename without extension).
    pub fn stem(&self) -> &str {
        let path = self.as_str();
        path.rsplit('/')
            .next()
            .and_then(|name| name.rsplit_once('.').map(|(stem, _)| stem))
            .unwrap_or(path)
    }

    /// Returns the parent folder path, or None if at root.
    pub fn parent(&self) -> Option<&str> {
        self.as_str().rsplit_once('/').map(|(parent, _)| parent)
    }

    /// Checks if this path is in the given folder (non-recursive).
    pub fn is_in_folder(&self, folder: &str) -> bool {
        let folder = folder.trim_matches('/');
        match self.parent() {
            Some(parent) => parent == folder,
            None => folder.is_empty(),
        }
    }

    /// Checks if this path is under the given folder (recursive).
    pub fn is_under_folder(&self, folder: &str) -> bool {
        let folder = folder.trim_matches('/');
        if folder.is_empty() {
            return true;
        }
        let path = self.as_str();
        path.starts_with(folder)
            && path.as_bytes().get(folder.len()).is_some_and(|&b| b == b'/')
    }

    /// Returns true if this is a Note variant.
    pub fn is_note(&self) -> bool {
        matches!(self, VaultPath::Note(_))
    }

    /// Returns true if this is an Image variant.
    pub fn is_image(&self) -> bool {
        matches!(self, VaultPath::Image(_))
    }

    /// Checks if this path ends with the given suffix.
    pub fn ends_with(&self, suffix: &str) -> bool {
        self.as_str().ends_with(suffix)
    }
}

/// Normalizes path separators to forward slashes.
fn normalize_separators(path: &str) -> String {
    let path = path.trim_start_matches(['/', '\\']);
    path.replace('\\', "/")
}

impl FromStr for VaultPath {
    type Err = VaultPathError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        VaultPath::new(s)
    }
}

impl fmt::Display for VaultPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl TryFrom<String> for VaultPath {
    type Error = VaultPathError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        VaultPath::new(value)
    }
}

impl TryFrom<&str> for VaultPath {
    type Error = VaultPathError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        VaultPath::new(value)
    }
}

impl TryFrom<PathBuf> for VaultPath {
    type Error = VaultPathError;

    fn try_from(value: PathBuf) -> Result<Self, Self::Error> {
        VaultPath::new(value.to_string_lossy())
    }
}

impl TryFrom<&Path> for VaultPath {
    type Error = VaultPathError;

    fn try_from(value: &Path) -> Result<Self, Self::Error> {
        VaultPath::new(value.to_string_lossy())
    }
}

impl Serialize for VaultPath {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for VaultPath {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        VaultPath::new(s).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_detects_note_variant() {
        let path = VaultPath::new("note.md").unwrap();
        assert!(matches!(path, VaultPath::Note(_)));
        assert_eq!(path.as_str(), "note.md");
        assert_eq!(path.stem(), "note");
    }

    #[test]
    fn auto_detects_image_variants() {
        for ext in ["png", "jpg", "jpeg", "gif", "bmp", "webp", "svg", "ico", "tiff", "tif"] {
            let path = VaultPath::new(format!("image.{ext}")).unwrap();
            assert!(matches!(path, VaultPath::Image(_)), "Failed for {ext}");
        }
    }

    #[test]
    fn auto_detects_any_variant() {
        let path = VaultPath::new("data.json").unwrap();
        assert!(matches!(path, VaultPath::Any(_)));

        let path = VaultPath::new("script.py").unwrap();
        assert!(matches!(path, VaultPath::Any(_)));
    }

    #[test]
    fn case_insensitive_extension() {
        let path = VaultPath::new("note.MD").unwrap();
        assert!(matches!(path, VaultPath::Note(_)));

        let path = VaultPath::new("image.PNG").unwrap();
        assert!(matches!(path, VaultPath::Image(_)));
    }

    #[test]
    fn valid_nested_path() {
        let path = VaultPath::new("projects/alpha/design.md").unwrap();
        assert_eq!(path.as_str(), "projects/alpha/design.md");
        assert_eq!(path.stem(), "design");
        assert_eq!(path.parent(), Some("projects/alpha"));
    }

    #[test]
    fn normalizes_backslashes() {
        let path = VaultPath::new(r"projects\alpha\design.md").unwrap();
        assert_eq!(path.as_str(), "projects/alpha/design.md");
    }

    #[test]
    fn normalizes_mixed_separators() {
        let path = VaultPath::new(r"projects/alpha\beta/note.md").unwrap();
        assert_eq!(path.as_str(), "projects/alpha/beta/note.md");
    }

    #[test]
    fn strips_leading_slash() {
        let path = VaultPath::new("/projects/note.md").unwrap();
        assert_eq!(path.as_str(), "projects/note.md");
    }

    #[test]
    fn strips_leading_backslash() {
        let path = VaultPath::new(r"\projects\note.md").unwrap();
        assert_eq!(path.as_str(), "projects/note.md");
    }

    #[test]
    fn rejects_empty_path() {
        let err = VaultPath::new("").unwrap_err();
        assert_eq!(err, VaultPathError::Empty);
    }

    #[test]
    fn rejects_path_traversal() {
        let err = VaultPath::new("../secret/note.md").unwrap_err();
        assert_eq!(err, VaultPathError::PathTraversal);

        let err = VaultPath::new("projects/../note.md").unwrap_err();
        assert_eq!(err, VaultPathError::PathTraversal);
    }

    #[test]
    fn is_in_folder_root() {
        let path = VaultPath::new("note.md").unwrap();
        assert!(path.is_in_folder(""));
        assert!(path.is_in_folder("/"));
        assert!(!path.is_in_folder("projects"));
    }

    #[test]
    fn is_in_folder_nested() {
        let path = VaultPath::new("projects/alpha/note.md").unwrap();
        assert!(path.is_in_folder("projects/alpha"));
        assert!(path.is_in_folder("/projects/alpha/"));
        assert!(!path.is_in_folder("projects"));
        assert!(!path.is_in_folder(""));
    }

    #[test]
    fn is_under_folder() {
        let path = VaultPath::new("projects/alpha/note.md").unwrap();
        assert!(path.is_under_folder(""));
        assert!(path.is_under_folder("projects"));
        assert!(path.is_under_folder("projects/alpha"));
        assert!(!path.is_under_folder("projects/beta"));
        assert!(!path.is_under_folder("other"));
    }

    #[test]
    fn is_under_folder_no_partial_match() {
        let path = VaultPath::new("projects-old/note.md").unwrap();
        assert!(!path.is_under_folder("projects"));
    }

    #[test]
    fn serde_roundtrip() {
        let path = VaultPath::new("projects/note.md").unwrap();
        let json = serde_json::to_string(&path).unwrap();
        assert_eq!(json, "\"projects/note.md\"");

        let parsed: VaultPath = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, path);
    }

    #[test]
    fn serde_rejects_invalid() {
        let result: Result<VaultPath, _> = serde_json::from_str("\"\"");
        assert!(result.is_err());
    }

    #[test]
    fn from_path_relative() {
        let root = Path::new("/vault");
        let path = Path::new("/vault/projects/note.md");
        let vault_path = VaultPath::from_path(path, root).unwrap();
        assert_eq!(vault_path.as_str(), "projects/note.md");
    }

    #[test]
    fn from_path_not_under_root() {
        let root = Path::new("/vault");
        let path = Path::new("/other/note.md");
        let err = VaultPath::from_path(path, root).unwrap_err();
        assert_eq!(err, VaultPathError::NotUnderRoot);
    }

    #[test]
    fn display_and_to_string() {
        let path = VaultPath::new("projects/note.md").unwrap();
        assert_eq!(path.to_string(), "projects/note.md");
        assert_eq!(format!("{path}"), "projects/note.md");
    }

    #[test]
    fn to_path_buf() {
        let path = VaultPath::new("projects/note.md").unwrap();
        let pb = path.as_path_buf();
        assert_eq!(pb, PathBuf::from("projects/note.md"));
    }

    #[test]
    fn parse_from_str() {
        let path: VaultPath = "daily/2024-03-14.md".parse().unwrap();
        assert_eq!(path.as_str(), "daily/2024-03-14.md");
    }

    #[test]
    fn try_from_implementations() {
        let from_string: VaultPath = String::from("note.md").try_into().unwrap();
        assert_eq!(from_string.as_str(), "note.md");

        let from_str: VaultPath = "note.md".try_into().unwrap();
        assert_eq!(from_str.as_str(), "note.md");

        let from_pathbuf: VaultPath = PathBuf::from("note.md").try_into().unwrap();
        assert_eq!(from_pathbuf.as_str(), "note.md");
    }

    #[test]
    fn is_note_and_is_image() {
        let note = VaultPath::new("test.md").unwrap();
        assert!(note.is_note());
        assert!(!note.is_image());

        let img = VaultPath::new("test.png").unwrap();
        assert!(!img.is_note());
        assert!(img.is_image());

        let other = VaultPath::new("test.json").unwrap();
        assert!(!other.is_note());
        assert!(!other.is_image());
    }
}
