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
    #[error("invalid sub-note path: {0}")]
    InvalidSubNotePath(String),
}

/// A validated, platform-independent vault path.
///
/// Each variant encodes the type of resource in the vault:
/// - `Root` — the vault root (empty string)
/// - `Folder(String)` — a folder, always ends with `/` (e.g. `"projects/alpha/"`)
/// - `Note(String)` — a markdown note with `.md` extension (e.g. `"projects/alpha.md"`)
/// - `Section(String)` — a section within a note (e.g. `"projects/alpha.md#Goals/Q1"`)
/// - `Block(String)` — a block reference within a note (e.g. `"note.md#^block-id"`)
/// - `Image(String)` — an image file (e.g. `"assets/logo.png"`)
/// - `Other(String)` — any other file type (e.g. `"data.json"`)
///
/// Internally stores paths with forward slashes (`/`) as separators,
/// regardless of the platform.
///
/// # Examples
///
/// ```
/// use tarn::common::VaultPath;
///
/// let note: VaultPath = "projects/alpha/design.md".try_into().unwrap();
/// assert!(note.is_note());
///
/// let section: VaultPath = "projects/alpha.md#Goals".try_into().unwrap();
/// assert!(section.is_section());
///
/// let folder: VaultPath = "projects/".try_into().unwrap();
/// assert!(folder.is_folder());
///
/// let root = VaultPath::new("").unwrap();
/// assert!(root.is_root());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum VaultPath {
    /// The vault root (empty string).
    Root,
    /// A folder path (ends with `/`).
    Folder(String),
    /// A markdown note (`.md` extension).
    Note(String),
    /// A section within a note (`note.md#Heading/Sub`).
    Section(String),
    /// A block reference within a note (`note.md#^block-id`).
    Block(String),
    /// An image file (png, jpg, gif, etc.).
    Image(String),
    /// Any other file type.
    Other(String),
}

/// Image extensions recognized by the vault.
const IMAGE_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "bmp", "webp", "svg", "ico", "tiff", "tif",
];

impl VaultPath {
    /// Creates a new VaultPath, validating the path and normalizing separators.
    ///
    /// An empty string represents the root folder.
    /// Bare separators like "/" or "\\" are rejected — only "" is valid root.
    /// Strings containing `#` are parsed as Section or Block paths.
    pub fn new(path: impl AsRef<str>) -> Result<Self, VaultPathError> {
        let path = path.as_ref();
        let normalized = normalize_separators(path);

        // Empty after normalization means root folder — but only if the
        // original input was truly empty. Bare separators like "/" are rejected.
        if normalized.is_empty() {
            if path.is_empty() {
                return Ok(VaultPath::Root);
            }
            return Err(VaultPathError::InvalidRoot);
        }

        if normalized.split('/').any(|segment| segment == "..") {
            return Err(VaultPathError::PathTraversal);
        }

        // Section or Block: contains `#` separator
        if normalized.contains('#') {
            let (note_part, fragment) = normalized
                .split_once('#')
                .expect("already checked contains #");

            // Validate the note portion is a valid note path
            if note_part.is_empty() || !note_part.ends_with(".md") {
                return Err(VaultPathError::InvalidSubNotePath(
                    "must reference a .md note".to_string(),
                ));
            }

            // Block reference: fragment starts with `^`
            if fragment.starts_with('^') {
                return Ok(VaultPath::Block(normalized));
            }

            return Ok(VaultPath::Section(normalized));
        }

        // Folder: ends with `/`
        if normalized.ends_with('/') {
            return Ok(VaultPath::Folder(normalized));
        }

        // Detect by extension
        match file_extension(&normalized) {
            Some("md") => Ok(VaultPath::Note(normalized)),
            Some(ext) if IMAGE_EXTENSIONS.contains(&ext) => Ok(VaultPath::Image(normalized)),
            _ => Ok(VaultPath::Other(normalized)),
        }
    }

    /// Returns true if this is the root folder.
    pub fn is_root(&self) -> bool {
        matches!(self, VaultPath::Root)
    }

    /// Returns true if this is a Folder (or Root).
    pub fn is_folder(&self) -> bool {
        matches!(self, VaultPath::Root | VaultPath::Folder(_))
    }

    /// Returns true if this is a Note.
    pub fn is_note(&self) -> bool {
        matches!(self, VaultPath::Note(_))
    }

    /// Returns true if this is a Section.
    pub fn is_section(&self) -> bool {
        matches!(self, VaultPath::Section(_))
    }

    /// Returns true if this is a Block reference.
    pub fn is_block(&self) -> bool {
        matches!(self, VaultPath::Block(_))
    }

    /// Returns true if this is an Image.
    pub fn is_image(&self) -> bool {
        matches!(self, VaultPath::Image(_))
    }

    /// Returns the normalized path as a string slice (always uses `/` separators).
    pub fn as_str(&self) -> &str {
        match self {
            VaultPath::Root => "",
            VaultPath::Folder(s)
            | VaultPath::Note(s)
            | VaultPath::Section(s)
            | VaultPath::Block(s)
            | VaultPath::Image(s)
            | VaultPath::Other(s) => s,
        }
    }

    /// Converts to a platform-native `PathBuf`.
    ///
    /// For Section and Block variants, returns the note portion only (before `#`).
    pub fn as_path_buf(&self) -> PathBuf {
        match self {
            VaultPath::Section(s) | VaultPath::Block(s) => {
                let note_part = s.split_once('#').map(|(p, _)| p).unwrap_or(s);
                PathBuf::from(note_part)
            }
            _ => PathBuf::from(self.as_str()),
        }
    }

    /// Creates a VaultPath from a filesystem Path, making it relative to the given root.
    ///
    /// Returns `NotUnderRoot` error if path is not under root.
    /// Never produces Section or Block variants (filesystem paths don't contain `#`).
    pub fn from_path(path: &Path, root: &Path) -> Result<Self, VaultPathError> {
        let relative = path
            .strip_prefix(root)
            .map_err(|_| VaultPathError::NotUnderRoot)?
            .to_string_lossy();

        VaultPath::new(relative)
    }

    /// Returns the file extension in lowercase, or None if no extension.
    ///
    /// For Section and Block variants, returns the extension of the note portion.
    pub fn extension(&self) -> Option<&str> {
        let path = match self {
            VaultPath::Root => return None,
            VaultPath::Section(s) | VaultPath::Block(s) => {
                s.split_once('#').map(|(p, _)| p).unwrap_or(s)
            }
            _ => self.as_str(),
        };
        file_extension(path)
    }

    /// Returns the file stem (filename without extension).
    ///
    /// For Section and Block variants, returns the stem of the note portion.
    pub fn stem(&self) -> &str {
        let path = match self {
            VaultPath::Root => return "",
            VaultPath::Section(s) | VaultPath::Block(s) => {
                s.split_once('#').map(|(p, _)| p).unwrap_or(s)
            }
            _ => self.as_str(),
        };
        let path = path.trim_end_matches('/');
        let name = path.rsplit('/').next().unwrap_or(path);
        name.rsplit_once('.').map(|(stem, _)| stem).unwrap_or(name)
    }

    /// Returns the parent path, or None if at root.
    ///
    /// Navigation hierarchy:
    /// - `Section("folder/note.md#heading")` → `Note("folder/note.md")`
    /// - `Block("folder/note.md#^id")` → `Note("folder/note.md")`
    /// - `Note("folder/note.md")` → `Folder("folder/")`
    /// - `Image("folder/img.png")` → `Folder("folder/")`
    /// - `Folder("projects/alpha/")` → `Folder("projects/")`
    /// - `Folder("projects/")` → `Root`
    /// - `Root` → `None`
    pub fn parent(&self) -> Option<VaultPath> {
        match self {
            VaultPath::Root => None,
            VaultPath::Section(s) | VaultPath::Block(s) => {
                let note_part = s.split_once('#').map(|(p, _)| p).unwrap_or(s);
                Some(VaultPath::Note(note_part.to_string()))
            }
            VaultPath::Folder(s) => {
                let path = s.trim_end_matches('/');
                match path.rsplit_once('/') {
                    Some((parent, _)) => Some(VaultPath::Folder(format!("{parent}/"))),
                    None => Some(VaultPath::Root),
                }
            }
            VaultPath::Note(s) | VaultPath::Image(s) | VaultPath::Other(s) => s
                .rsplit_once('/')
                .map(|(parent, _)| VaultPath::Folder(format!("{parent}/"))),
        }
    }

    /// For Section/Block, returns the Note portion. For Note, returns self clone. Others return None.
    pub fn note_path(&self) -> Option<VaultPath> {
        match self {
            VaultPath::Section(s) | VaultPath::Block(s) => {
                let note_part = s.split_once('#').map(|(p, _)| p).unwrap_or(s);
                Some(VaultPath::Note(note_part.to_string()))
            }
            VaultPath::Note(_) => Some(self.clone()),
            _ => None,
        }
    }

    /// For Block, returns the block identifier (without `^`). Others return None.
    ///
    /// Example: `VaultPath::Block("note.md#^quote-of-the-day")` → `Some("quote-of-the-day")`
    pub fn block_id(&self) -> Option<&str> {
        match self {
            VaultPath::Block(s) => {
                let (_, fragment) = s.split_once('#').unwrap_or(("", ""));
                Some(fragment.trim_start_matches('^'))
            }
            _ => None,
        }
    }

    /// For Section, returns the heading path components. Others return empty vec.
    ///
    /// Example: `VaultPath::Section("note.md#Goals/Q1")` → `["Goals", "Q1"]`
    pub fn section_headings(&self) -> Vec<String> {
        match self {
            VaultPath::Section(s) => {
                let (_, section) = s.split_once('#').unwrap_or(("", ""));
                if section.is_empty() {
                    Vec::new()
                } else {
                    section.split('/').map(String::from).collect()
                }
            }
            _ => Vec::new(),
        }
    }

    /// Checks if this path is in the given folder (non-recursive).
    pub fn is_in_folder(&self, folder: &VaultPath) -> bool {
        self.parent().as_ref() == Some(folder)
    }

    /// Checks if this path is under the given folder (recursive).
    ///
    /// For Section and Block variants, checks based on the note path portion.
    pub fn is_under_folder(&self, folder: &VaultPath) -> bool {
        if !folder.is_folder() {
            return false;
        }

        // Get the file-path portion (for Section/Block, use only the note part)
        let path = match self {
            VaultPath::Root => return false,
            VaultPath::Section(s) | VaultPath::Block(s) => {
                s.split_once('#').map(|(p, _)| p).unwrap_or(s)
            }
            _ => self.as_str(),
        };

        let folder_path = match folder {
            VaultPath::Root => return true,
            VaultPath::Folder(f) => f.trim_end_matches('/'),
            _ => unreachable!("checked is_folder above"),
        };

        if folder_path.is_empty() {
            return true;
        }

        let path = path.trim_end_matches('/');
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

/// Extracts the file extension from a path string.
fn file_extension(path: &str) -> Option<&str> {
    let path = path.trim_end_matches('/');
    let filename = path.rsplit('/').next()?;
    let (_, ext) = filename.rsplit_once('.')?;
    // Avoid treating hidden files like ".gitignore" as having extension "gitignore"
    if filename.starts_with('.') && !filename[1..].contains('.') {
        return None;
    }
    Some(ext)
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

impl PartialOrd for VaultPath {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for VaultPath {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_str().cmp(other.as_str())
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
    fn detects_note() {
        let path = VaultPath::new("note.md").unwrap();
        assert!(path.is_note());
        assert!(!path.is_folder());
        assert!(!path.is_image());
        assert!(!path.is_section());
        assert_eq!(path.as_str(), "note.md");
        assert_eq!(path.stem(), "note");
    }

    #[test]
    fn detects_image_kinds() {
        for ext in [
            "png", "jpg", "jpeg", "gif", "bmp", "webp", "svg", "ico", "tiff", "tif",
        ] {
            let path = VaultPath::new(format!("image.{ext}")).unwrap();
            assert!(path.is_image(), "Failed for {ext}");
        }
    }

    #[test]
    fn detects_folder() {
        let path = VaultPath::new("projects/").unwrap();
        assert!(path.is_folder());
        assert!(!path.is_root());

        let nested = VaultPath::new("projects/alpha/").unwrap();
        assert!(nested.is_folder());
    }

    #[test]
    fn detects_other() {
        let path = VaultPath::new("data.json").unwrap();
        assert!(matches!(path, VaultPath::Other(_)));

        let path = VaultPath::new("script.py").unwrap();
        assert!(matches!(path, VaultPath::Other(_)));
    }

    #[test]
    fn detects_section() {
        let path = VaultPath::new("note.md#Goals").unwrap();
        assert!(path.is_section());
        assert_eq!(path.as_str(), "note.md#Goals");

        let path = VaultPath::new("folder/note.md#Goals/Q1").unwrap();
        assert!(path.is_section());
        assert_eq!(path.section_headings(), vec!["Goals", "Q1"]);

        // Root section (no heading)
        let path = VaultPath::new("note.md#").unwrap();
        assert!(path.is_section());
        assert!(path.section_headings().is_empty());
    }

    #[test]
    fn detects_block() {
        let path = VaultPath::new("note.md#^block-id").unwrap();
        assert!(path.is_block());
        assert!(!path.is_section());
        assert_eq!(path.as_str(), "note.md#^block-id");
        assert_eq!(path.block_id(), Some("block-id"));

        let path = VaultPath::new("folder/note.md#^quote-of-the-day").unwrap();
        assert!(path.is_block());
        assert_eq!(path.block_id(), Some("quote-of-the-day"));

        // Block with just caret and id
        let path = VaultPath::new("note.md#^37066d").unwrap();
        assert!(path.is_block());
        assert_eq!(path.block_id(), Some("37066d"));
    }

    #[test]
    fn block_note_path() {
        let block = VaultPath::new("folder/note.md#^block-id").unwrap();
        assert_eq!(
            block.note_path(),
            Some(VaultPath::new("folder/note.md").unwrap())
        );
    }

    #[test]
    fn block_parent_is_note() {
        let block = VaultPath::new("folder/note.md#^block-id").unwrap();
        assert_eq!(
            block.parent(),
            Some(VaultPath::new("folder/note.md").unwrap())
        );
    }

    #[test]
    fn block_path_buf_returns_note() {
        let block = VaultPath::new("folder/note.md#^block-id").unwrap();
        assert_eq!(block.as_path_buf(), PathBuf::from("folder/note.md"));
    }

    #[test]
    fn block_stem_and_extension() {
        let block = VaultPath::new("folder/note.md#^block-id").unwrap();
        assert_eq!(block.stem(), "note");
        assert_eq!(block.extension(), Some("md"));
    }

    #[test]
    fn block_is_under_folder() {
        let block = VaultPath::new("projects/note.md#^block-id").unwrap();
        let projects = VaultPath::new("projects/").unwrap();
        assert!(block.is_under_folder(&projects));

        let other = VaultPath::new("other/").unwrap();
        assert!(!block.is_under_folder(&other));
    }

    #[test]
    fn block_section_headings_empty() {
        let block = VaultPath::new("note.md#^block-id").unwrap();
        assert!(block.section_headings().is_empty());
    }

    #[test]
    fn block_id_none_for_non_block() {
        let note = VaultPath::new("note.md").unwrap();
        assert_eq!(note.block_id(), None);

        let section = VaultPath::new("note.md#Goals").unwrap();
        assert_eq!(section.block_id(), None);
    }

    #[test]
    fn serde_roundtrip_block() {
        let block = VaultPath::new("note.md#^block-id").unwrap();
        let json = serde_json::to_string(&block).unwrap();
        assert_eq!(json, "\"note.md#^block-id\"");

        let parsed: VaultPath = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, block);
        assert!(parsed.is_block());
    }

    #[test]
    fn sub_note_path_requires_md_note() {
        let err = VaultPath::new("data.json#something").unwrap_err();
        assert!(matches!(err, VaultPathError::InvalidSubNotePath(_)));

        let err = VaultPath::new("#heading").unwrap_err();
        assert!(matches!(err, VaultPathError::InvalidSubNotePath(_)));

        let err = VaultPath::new("data.json#^block").unwrap_err();
        assert!(matches!(err, VaultPathError::InvalidSubNotePath(_)));
    }

    #[test]
    fn section_note_path() {
        let section = VaultPath::new("folder/note.md#Goals/Q1").unwrap();
        assert_eq!(
            section.note_path(),
            Some(VaultPath::new("folder/note.md").unwrap())
        );

        let note = VaultPath::new("folder/note.md").unwrap();
        assert_eq!(note.note_path(), Some(note.clone()));

        let folder = VaultPath::new("folder/").unwrap();
        assert_eq!(folder.note_path(), None);
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
        // Section returns note's extension
        assert_eq!(
            VaultPath::new("note.md#Heading").unwrap().extension(),
            Some("md")
        );
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
    fn parent_hierarchy() {
        // Section -> Note
        let section = VaultPath::new("folder/note.md#Goals/Q1").unwrap();
        assert_eq!(
            section.parent(),
            Some(VaultPath::new("folder/note.md").unwrap())
        );

        // Note -> Folder
        let note = VaultPath::new("folder/note.md").unwrap();
        assert_eq!(note.parent(), Some(VaultPath::new("folder/").unwrap()));

        // Note at root level -> None
        let root_file = VaultPath::new("note.md").unwrap();
        assert_eq!(root_file.parent(), None);

        // Folder -> parent Folder
        let folder = VaultPath::new("projects/alpha/").unwrap();
        assert_eq!(folder.parent(), Some(VaultPath::new("projects/").unwrap()));

        // Top-level Folder -> Root
        let top_folder = VaultPath::new("projects/").unwrap();
        assert_eq!(top_folder.parent(), Some(VaultPath::Root));

        // Root -> None
        assert_eq!(VaultPath::Root.parent(), None);

        // Image -> Folder
        let img = VaultPath::new("assets/logo.png").unwrap();
        assert_eq!(img.parent(), Some(VaultPath::new("assets/").unwrap()));
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
    fn section_is_under_folder() {
        let section = VaultPath::new("projects/alpha/note.md#Goals").unwrap();
        let projects = VaultPath::new("projects/").unwrap();
        assert!(section.is_under_folder(&projects));

        let other = VaultPath::new("other/").unwrap();
        assert!(!section.is_under_folder(&other));
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
    fn serde_roundtrip_section() {
        let section = VaultPath::new("note.md#Goals/Q1").unwrap();
        let json = serde_json::to_string(&section).unwrap();
        assert_eq!(json, "\"note.md#Goals/Q1\"");

        let parsed: VaultPath = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, section);
        assert!(parsed.is_section());
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

        // Section returns note portion only
        let section = VaultPath::new("projects/note.md#Goals").unwrap();
        assert_eq!(section.as_path_buf(), PathBuf::from("projects/note.md"));
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
    fn section_stem() {
        let section = VaultPath::new("projects/note.md#Goals").unwrap();
        assert_eq!(section.stem(), "note");
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
