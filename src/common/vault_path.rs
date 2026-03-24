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
    #[error("only empty string represents root, '/' is not allowed")]
    InvalidRoot,
    #[error("path is not under root")]
    NotUnderRoot,
}

/// The kind of path in a vault.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PathKind {
    /// Markdown note (.md extension)
    Note,
    /// Image file (png, jpg, gif, etc.)
    Image,
    /// Folder (ends with /)
    Folder,
    /// Any other file type
    Other,
}

/// A validated, platform-independent vault path.
///
/// Internally stores the path with forward slashes (`/`) as separators,
/// regardless of the platform. This ensures consistent behavior for shared
/// indexes across Unix and Windows systems.
///
/// The path kind is determined by:
/// - Trailing `/` -> `PathKind::Folder`
/// - `.md` extension -> `PathKind::Note`
/// - Image extensions (.png, .jpg, etc.) -> `PathKind::Image`
/// - Anything else -> `PathKind::Other`
///
/// # Examples
///
/// ```
/// use tarn::common::{VaultPath, PathKind};
///
/// let note: VaultPath = "projects/alpha/design.md".try_into().unwrap();
/// assert_eq!(note.kind(), PathKind::Note);
///
/// let img: VaultPath = "assets/logo.png".try_into().unwrap();
/// assert_eq!(img.kind(), PathKind::Image);
///
/// let folder: VaultPath = "projects/".try_into().unwrap();
/// assert_eq!(folder.kind(), PathKind::Folder);
///
/// let other: VaultPath = "data.json".try_into().unwrap();
/// assert_eq!(other.kind(), PathKind::Other);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct VaultPath(String);

impl VaultPath {
    /// Creates a new VaultPath, validating the path and normalizing separators.
    ///
    /// An empty string represents the root folder.
    /// Bare separators like "/" or "\\" are rejected — only "" is valid root.
    pub fn new(path: impl AsRef<str>) -> Result<Self, VaultPathError> {
        let path = path.as_ref();
        let normalized = normalize_separators(path);

        // Empty after normalization means root folder — but only if the
        // original input was truly empty. Bare separators like "/" are rejected.
        if normalized.is_empty() {
            if path.is_empty() {
                return Ok(VaultPath(String::new()));
            }
            return Err(VaultPathError::InvalidRoot);
        }

        if normalized.split('/').any(|segment| segment == "..") {
            return Err(VaultPathError::PathTraversal);
        }

        Ok(VaultPath(normalized))
    }

    /// Returns true if this is the root folder.
    pub fn is_root(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns the normalized path as a string slice (always uses `/` separators).
    pub fn as_str(&self) -> &str {
        &self.0
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

    /// Returns the kind of this path.
    pub fn kind(&self) -> PathKind {
        // Root or trailing slash means folder
        if self.0.is_empty() || self.0.ends_with('/') {
            return PathKind::Folder;
        }

        match self.extension() {
            Some("md") => PathKind::Note,
            Some(
                "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "svg" | "ico" | "tiff" | "tif",
            ) => PathKind::Image,
            _ => PathKind::Other,
        }
    }

    /// Returns the file extension in lowercase, or None if no extension.
    pub fn extension(&self) -> Option<&str> {
        let path = self.0.trim_end_matches('/');
        let filename = path.rsplit('/').next()?;
        let (_, ext) = filename.rsplit_once('.')?;
        // Avoid treating hidden files like ".gitignore" as having extension "gitignore"
        if filename.starts_with('.') && !filename[1..].contains('.') {
            return None;
        }
        Some(ext)
    }

    /// Returns true if this is a Note.
    pub fn is_note(&self) -> bool {
        matches!(self.kind(), PathKind::Note)
    }

    /// Returns true if this is an Image.
    pub fn is_image(&self) -> bool {
        matches!(self.kind(), PathKind::Image)
    }

    /// Returns true if this is a Folder.
    pub fn is_folder(&self) -> bool {
        matches!(self.kind(), PathKind::Folder)
    }

    /// Returns the file stem (filename without extension).
    pub fn stem(&self) -> &str {
        let path = self.0.trim_end_matches('/');
        let name = path.rsplit('/').next().unwrap_or(path);
        name.rsplit_once('.').map(|(stem, _)| stem).unwrap_or(name)
    }

    /// Returns the parent folder path, or None if at root.
    pub fn parent(&self) -> Option<VaultPath> {
        let path = self.0.trim_end_matches('/');
        path.rsplit_once('/').map(|(parent, _)| {
            // Parent is always a folder
            VaultPath(format!("{parent}/"))
        })
    }

    /// Checks if this path is in the given folder (non-recursive).
    pub fn is_in_folder(&self, folder: &VaultPath) -> bool {
        self.parent().as_ref() == Some(folder)
    }

    /// Checks if this path is under the given folder (recursive).
    pub fn is_under_folder(&self, folder: &VaultPath) -> bool {
        if !folder.is_folder() {
            return false;
        }
        let folder_path = folder.0.trim_end_matches('/');
        if folder_path.is_empty() {
            return true;
        }
        let path = self.0.trim_end_matches('/');
        path.starts_with(folder_path)
            && path
                .as_bytes()
                .get(folder_path.len())
                .is_some_and(|&b| b == b'/')
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
        f.write_str(&self.0)
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
        serializer.serialize_str(&self.0)
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
    fn detects_note_kind() {
        let path = VaultPath::new("note.md").unwrap();
        assert_eq!(path.kind(), PathKind::Note);
        assert!(path.is_note());
        assert_eq!(path.as_str(), "note.md");
        assert_eq!(path.stem(), "note");
    }

    #[test]
    fn detects_image_kinds() {
        for ext in [
            "png", "jpg", "jpeg", "gif", "bmp", "webp", "svg", "ico", "tiff", "tif",
        ] {
            let path = VaultPath::new(format!("image.{ext}")).unwrap();
            assert_eq!(path.kind(), PathKind::Image, "Failed for {ext}");
            assert!(path.is_image());
        }
    }

    #[test]
    fn detects_folder_kind() {
        let path = VaultPath::new("projects/").unwrap();
        assert_eq!(path.kind(), PathKind::Folder);
        assert!(path.is_folder());

        let nested = VaultPath::new("projects/alpha/").unwrap();
        assert_eq!(nested.kind(), PathKind::Folder);
    }

    #[test]
    fn detects_other_kind() {
        let path = VaultPath::new("data.json").unwrap();
        assert_eq!(path.kind(), PathKind::Other);

        let path = VaultPath::new("script.py").unwrap();
        assert_eq!(path.kind(), PathKind::Other);
    }

    #[test]
    fn extension_detection() {
        assert_eq!(VaultPath::new("note.md").unwrap().extension(), Some("md"));
        assert_eq!(
            VaultPath::new("file.tar.gz").unwrap().extension(),
            Some("gz")
        );
        assert_eq!(VaultPath::new("noext").unwrap().extension(), None);
        assert_eq!(VaultPath::new(".gitignore").unwrap().extension(), None);
        assert_eq!(
            VaultPath::new(".config.json").unwrap().extension(),
            Some("json")
        );
        assert_eq!(VaultPath::new("folder/").unwrap().extension(), None);
    }

    #[test]
    fn valid_nested_path() {
        let path = VaultPath::new("projects/alpha/design.md").unwrap();
        assert_eq!(path.as_str(), "projects/alpha/design.md");
        assert_eq!(path.stem(), "design");
        assert_eq!(
            path.parent(),
            Some(VaultPath::new("projects/alpha/").unwrap())
        );
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
    fn root_folder() {
        let root = VaultPath::new("").unwrap();
        assert!(root.is_root());
        assert!(root.is_folder());
        assert_eq!(root.kind(), PathKind::Folder);
        assert_eq!(root.as_str(), "");
    }

    #[test]
    fn rejects_bare_separator_as_root() {
        assert_eq!(
            VaultPath::new("/").unwrap_err(),
            VaultPathError::InvalidRoot
        );
        assert_eq!(
            VaultPath::new("\\").unwrap_err(),
            VaultPathError::InvalidRoot
        );
        assert_eq!(
            VaultPath::new("///").unwrap_err(),
            VaultPathError::InvalidRoot
        );
        assert_eq!(
            VaultPath::new("\\\\/").unwrap_err(),
            VaultPathError::InvalidRoot
        );
    }

    #[test]
    fn rejects_path_traversal() {
        let err = VaultPath::new("../secret/note.md").unwrap_err();
        assert_eq!(err, VaultPathError::PathTraversal);

        let err = VaultPath::new("projects/../note.md").unwrap_err();
        assert_eq!(err, VaultPathError::PathTraversal);
    }

    #[test]
    fn parent_returns_folder() {
        let path = VaultPath::new("projects/alpha/note.md").unwrap();
        let parent = path.parent().unwrap();
        assert!(parent.is_folder());
        assert_eq!(parent.as_str(), "projects/alpha/");

        let root_file = VaultPath::new("note.md").unwrap();
        assert_eq!(root_file.parent(), None);

        let folder = VaultPath::new("projects/alpha/").unwrap();
        let folder_parent = folder.parent().unwrap();
        assert_eq!(folder_parent.as_str(), "projects/");
    }

    #[test]
    fn is_in_folder() {
        let root_folder = VaultPath::new("projects/alpha/").unwrap();
        let path = VaultPath::new("projects/alpha/note.md").unwrap();
        assert!(path.is_in_folder(&root_folder));

        let other_folder = VaultPath::new("projects/").unwrap();
        assert!(!path.is_in_folder(&other_folder));
    }

    #[test]
    fn is_under_folder() {
        let projects = VaultPath::new("projects/").unwrap();
        let alpha = VaultPath::new("projects/alpha/").unwrap();
        let path = VaultPath::new("projects/alpha/note.md").unwrap();

        assert!(path.is_under_folder(&projects));
        assert!(path.is_under_folder(&alpha));

        let beta = VaultPath::new("projects/beta/").unwrap();
        assert!(!path.is_under_folder(&beta));

        let other = VaultPath::new("other/").unwrap();
        assert!(!path.is_under_folder(&other));
    }

    #[test]
    fn is_under_folder_no_partial_match() {
        let path = VaultPath::new("projects-old/note.md").unwrap();
        let projects = VaultPath::new("projects/").unwrap();
        assert!(!path.is_under_folder(&projects));
    }

    #[test]
    fn is_under_folder_requires_folder() {
        let path = VaultPath::new("projects/note.md").unwrap();
        let file = VaultPath::new("projects").unwrap();
        assert!(!path.is_under_folder(&file));
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
    fn serde_roundtrip_folder() {
        let path = VaultPath::new("projects/").unwrap();
        let json = serde_json::to_string(&path).unwrap();
        assert_eq!(json, "\"projects/\"");

        let parsed: VaultPath = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, path);
        assert!(parsed.is_folder());
    }

    #[test]
    fn serde_rejects_invalid() {
        // Path traversal is invalid
        let result: Result<VaultPath, _> = serde_json::from_str("\"../secret\"");
        assert!(result.is_err());
    }

    #[test]
    fn serde_roundtrip_root() {
        let root = VaultPath::new("").unwrap();
        let json = serde_json::to_string(&root).unwrap();
        assert_eq!(json, "\"\"");

        let parsed: VaultPath = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, root);
        assert!(parsed.is_root());
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
    fn is_note_image_folder() {
        let note = VaultPath::new("test.md").unwrap();
        assert!(note.is_note());
        assert!(!note.is_image());
        assert!(!note.is_folder());

        let img = VaultPath::new("test.png").unwrap();
        assert!(!img.is_note());
        assert!(img.is_image());
        assert!(!img.is_folder());

        let folder = VaultPath::new("test/").unwrap();
        assert!(!folder.is_note());
        assert!(!folder.is_image());
        assert!(folder.is_folder());

        let other = VaultPath::new("test.json").unwrap();
        assert!(!other.is_note());
        assert!(!other.is_image());
        assert!(!other.is_folder());
    }

    #[test]
    fn folder_stem() {
        let folder = VaultPath::new("projects/").unwrap();
        assert_eq!(folder.stem(), "projects");

        let nested = VaultPath::new("projects/alpha/").unwrap();
        assert_eq!(nested.stem(), "alpha");
    }

    #[test]
    fn ends_with_suffix() {
        let path = VaultPath::new("projects/design.md").unwrap();
        assert!(path.ends_with(".md"));
        assert!(path.ends_with("design.md"));
        assert!(!path.ends_with(".txt"));
    }

    #[test]
    fn is_under_root_folder() {
        let root = VaultPath::new("").unwrap();
        let path = VaultPath::new("projects/note.md").unwrap();
        assert!(path.is_under_folder(&root));

        let folder = VaultPath::new("projects/").unwrap();
        assert!(folder.is_under_folder(&root));
    }

    #[test]
    fn try_from_path_ref() {
        let path = std::path::Path::new("note.md");
        let vault_path: VaultPath = path.try_into().unwrap();
        assert_eq!(vault_path.as_str(), "note.md");
    }
}
