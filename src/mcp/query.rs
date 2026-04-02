//! Query parser for extracting structured filters from raw query strings.
//!
//! Supports `tag:name` and `folder:path` inline tokens. Everything else
//! is treated as free-text for BM25 search.

use crate::common::VaultPath;

/// Result of parsing a raw query string.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParsedQuery {
    /// Remaining text after extracting filter tokens.
    pub text: String,
    /// Tags extracted from `tag:name` tokens.
    pub tags: Vec<String>,
    /// Folders extracted from `folder:path` tokens.
    pub folders: Vec<VaultPath>,
}

impl From<&str> for ParsedQuery {
    /// Parse a raw query string into structured parts.
    ///
    /// Extracts `tag:name` and `folder:path` tokens as hard filters.
    /// The remainder is joined back as the text query for BM25.
    ///
    /// # Examples
    ///
    /// ```
    /// use tarn::mcp::query::ParsedQuery;
    ///
    /// let parsed = ParsedQuery::from("event sourcing tag:architecture folder:concepts/");
    /// assert_eq!(parsed.text, "event sourcing");
    /// assert_eq!(parsed.tags, vec!["architecture"]);
    /// assert_eq!(parsed.folders.len(), 1);
    /// ```
    fn from(raw: &str) -> Self {
        let mut text_parts = Vec::new();
        let mut tags = Vec::new();
        let mut folders = Vec::new();

        for token in raw.split_whitespace() {
            if let Some(tag) = token.strip_prefix("tag:") {
                if !tag.is_empty() {
                    tags.push(tag.to_string());
                }
            } else if let Some(folder) = token.strip_prefix("folder:") {
                if !folder.is_empty() {
                    // Ensure folder path ends with /
                    let folder_str = if folder.ends_with('/') {
                        folder.to_string()
                    } else {
                        format!("{folder}/")
                    };
                    if let Ok(path) = VaultPath::new(folder_str) {
                        folders.push(path);
                    }
                }
            } else {
                text_parts.push(token);
            }
        }

        Self {
            text: text_parts.join(" "),
            tags,
            folders,
        }
    }
}

impl From<String> for ParsedQuery {
    fn from(raw: String) -> Self {
        Self::from(raw.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_query() {
        let parsed = ParsedQuery::from("");
        assert_eq!(parsed.text, "");
        assert!(parsed.tags.is_empty());
        assert!(parsed.folders.is_empty());
    }

    #[test]
    fn text_only() {
        let parsed = ParsedQuery::from("event sourcing patterns");
        assert_eq!(parsed.text, "event sourcing patterns");
        assert!(parsed.tags.is_empty());
        assert!(parsed.folders.is_empty());
    }

    #[test]
    fn single_tag() {
        let parsed = ParsedQuery::from("rust tag:programming");
        assert_eq!(parsed.text, "rust");
        assert_eq!(parsed.tags, vec!["programming"]);
    }

    #[test]
    fn multiple_tags() {
        let parsed = ParsedQuery::from("tag:rust tag:web");
        assert_eq!(parsed.text, "");
        assert_eq!(parsed.tags, vec!["rust", "web"]);
    }

    #[test]
    fn hierarchical_tag() {
        let parsed = ParsedQuery::from("tag:project/alpha");
        assert_eq!(parsed.tags, vec!["project/alpha"]);
    }

    #[test]
    fn single_folder() {
        let parsed = ParsedQuery::from("search text folder:concepts/");
        assert_eq!(parsed.text, "search text");
        assert_eq!(parsed.folders.len(), 1);
        assert_eq!(parsed.folders[0].to_string(), "concepts/");
    }

    #[test]
    fn folder_without_trailing_slash() {
        let parsed = ParsedQuery::from("folder:projects");
        assert_eq!(parsed.folders.len(), 1);
        assert_eq!(parsed.folders[0].to_string(), "projects/");
    }

    #[test]
    fn mixed_filters_and_text() {
        let parsed = ParsedQuery::from("event sourcing tag:architecture folder:concepts/ patterns");
        assert_eq!(parsed.text, "event sourcing patterns");
        assert_eq!(parsed.tags, vec!["architecture"]);
        assert_eq!(parsed.folders.len(), 1);
    }

    #[test]
    fn tag_only_no_text() {
        let parsed = ParsedQuery::from("tag:rust tag:systems");
        assert_eq!(parsed.text, "");
        assert_eq!(parsed.tags, vec!["rust", "systems"]);
    }

    #[test]
    fn empty_tag_value_ignored() {
        let parsed = ParsedQuery::from("tag: hello");
        assert_eq!(parsed.text, "hello");
        assert!(parsed.tags.is_empty());
    }

    #[test]
    fn empty_folder_value_ignored() {
        let parsed = ParsedQuery::from("folder: hello");
        assert_eq!(parsed.text, "hello");
        assert!(parsed.folders.is_empty());
    }

    #[test]
    fn multiple_folders() {
        let parsed = ParsedQuery::from("folder:a/ folder:b/");
        assert_eq!(parsed.folders.len(), 2);
    }
}
