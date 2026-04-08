//! MCP request types.

use std::sync::LazyLock;

use regex::Regex;
use schemars::JsonSchema;
use serde::{Deserialize, Deserializer};

use crate::common::VaultPath;

/// Regex for parsing search query tokens.
///
/// Matches in order:
/// - `tag:` with quoted value, closing quote optional (group 1)
/// - `tag:` with unquoted value (group 2)
/// - `folder:` with quoted value, closing quote optional (group 3)
/// - `folder:` with unquoted value (group 4)
/// - Quoted plain text, closing quote optional (group 5)
/// - Plain word (group 6)
static QUERY_TOKEN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?x)
        tag:"([^"]*)"?
      | tag:(\S+)
      | folder:"([^"]*)"?
      | folder:(\S+)
      | "([^"]*)"?
      | (\S+)
    "#,
    )
    .unwrap()
});

// ---------------------------------------------------------------------------
// SearchQuery
// ---------------------------------------------------------------------------

/// Parsed search query with extracted filters.
///
/// Supports `tag:name` and `folder:path` inline tokens. Everything else
/// is treated as free-text for BM25 search.
///
/// # Examples
///
/// ```
/// use tarn::mcp::types::SearchQuery;
///
/// let query = SearchQuery::from("event sourcing tag:architecture folder:concepts/");
/// assert_eq!(query.text, "event sourcing");
/// assert_eq!(query.tags, vec!["architecture"]);
/// assert_eq!(query.folders.len(), 1);
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SearchQuery {
    /// Remaining text after extracting filter tokens.
    pub text: String,
    /// Tags extracted from `tag:name` tokens.
    pub tags: Vec<String>,
    /// Folders extracted from `folder:path` tokens.
    pub folders: Vec<VaultPath>,
}

impl From<&str> for SearchQuery {
    /// Parse a raw query string into structured parts.
    ///
    /// Extracts `tag:name` and `folder:path` tokens as hard filters.
    /// The remainder is joined back as the text query for BM25.
    fn from(raw: &str) -> Self {
        let mut text_parts = Vec::new();
        let mut tags = Vec::new();
        let mut folders = Vec::new();

        for caps in QUERY_TOKEN_RE.captures_iter(raw) {
            if let Some(m) = caps.get(1).or_else(|| caps.get(2)) {
                // tag: filter (quoted or unquoted)
                if !m.as_str().is_empty() {
                    tags.push(m.as_str().to_string());
                }
            } else if let Some(m) = caps.get(3).or_else(|| caps.get(4)) {
                // folder: filter (quoted or unquoted)
                let folder = m.as_str();
                if !folder.is_empty() {
                    let folder_str = if folder.ends_with('/') {
                        folder.to_string()
                    } else {
                        format!("{folder}/")
                    };
                    if let Ok(path) = VaultPath::new(folder_str) {
                        folders.push(path);
                    }
                }
            } else if let Some(m) = caps.get(5).or_else(|| caps.get(6)) {
                // plain text (quoted or unquoted)
                text_parts.push(m.as_str().to_string());
            }
        }

        Self {
            text: text_parts.join(" "),
            tags,
            folders,
        }
    }
}

impl From<String> for SearchQuery {
    fn from(raw: String) -> Self {
        Self::from(raw.as_str())
    }
}

impl<'de> Deserialize<'de> for SearchQuery {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(SearchQuery::from(s))
    }
}

impl JsonSchema for SearchQuery {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "SearchQuery".into()
    }

    fn json_schema(_gen: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "string",
            "description": "Search query string. Supports tag:name and folder:path inline filters."
        })
    }
}

// ---------------------------------------------------------------------------
// Tool request types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchParams {
    #[schemars(
        description = "Search query. Supports tag:name and folder:path inline filters. Omit to list notes."
    )]
    pub query: Option<SearchQuery>,
    #[schemars(description = "Max note results (default: 20)")]
    pub limit: Option<usize>,
    #[schemars(description = "Max total tokens across all results")]
    pub token_limit: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetTagsParams {
    #[schemars(description = "Filter tags by prefix (e.g. \"project/\")")]
    pub prefix: Option<String>,
    #[schemars(description = "Include list of notes per tag (default: false)")]
    pub include_notes: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateNoteParams {
    #[schemars(description = "Note path (e.g. \"projects/alpha/design.md\")")]
    pub path: String,
    #[schemars(description = "Markdown content for the new note")]
    pub content: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateNoteParams {
    #[schemars(description = "Note path (e.g. \"projects/alpha/design.md\")")]
    pub path: String,
    #[schemars(description = "New markdown content for the note")]
    pub content: String,
    #[schemars(description = "Revision token from a prior read for conflict detection")]
    pub revision: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReplaceInNoteParams {
    #[schemars(description = "Note path (e.g. \"projects/alpha/design.md\")")]
    pub path: String,
    #[schemars(description = "Text or regex pattern to find")]
    pub old: String,
    #[schemars(description = "Replacement text")]
    pub new: String,
    #[schemars(description = "Replacement mode: \"first\" (default), \"all\", or \"regex\"")]
    pub mode: Option<String>,
    #[schemars(description = "Revision token from a prior read for conflict detection")]
    pub revision: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_query() {
        let parsed = SearchQuery::from("");
        assert_eq!(parsed.text, "");
        assert!(parsed.tags.is_empty());
        assert!(parsed.folders.is_empty());
    }

    #[test]
    fn text_only() {
        let parsed = SearchQuery::from("event sourcing patterns");
        assert_eq!(parsed.text, "event sourcing patterns");
        assert!(parsed.tags.is_empty());
        assert!(parsed.folders.is_empty());
    }

    #[test]
    fn single_tag() {
        let parsed = SearchQuery::from("rust tag:programming");
        assert_eq!(parsed.text, "rust");
        assert_eq!(parsed.tags, vec!["programming"]);
    }

    #[test]
    fn multiple_tags() {
        let parsed = SearchQuery::from("tag:rust tag:web");
        assert_eq!(parsed.text, "");
        assert_eq!(parsed.tags, vec!["rust", "web"]);
    }

    #[test]
    fn hierarchical_tag() {
        let parsed = SearchQuery::from("tag:project/alpha");
        assert_eq!(parsed.tags, vec!["project/alpha"]);
    }

    #[test]
    fn single_folder() {
        let parsed = SearchQuery::from("search text folder:concepts/");
        assert_eq!(parsed.text, "search text");
        assert_eq!(parsed.folders.len(), 1);
        assert_eq!(parsed.folders[0].to_string(), "concepts/");
    }

    #[test]
    fn folder_without_trailing_slash() {
        let parsed = SearchQuery::from("folder:projects");
        assert_eq!(parsed.folders.len(), 1);
        assert_eq!(parsed.folders[0].to_string(), "projects/");
    }

    #[test]
    fn mixed_filters_and_text() {
        let parsed = SearchQuery::from("event sourcing tag:architecture folder:concepts/ patterns");
        assert_eq!(parsed.text, "event sourcing patterns");
        assert_eq!(parsed.tags, vec!["architecture"]);
        assert_eq!(parsed.folders.len(), 1);
    }

    #[test]
    fn tag_only_no_text() {
        let parsed = SearchQuery::from("tag:rust tag:systems");
        assert_eq!(parsed.text, "");
        assert_eq!(parsed.tags, vec!["rust", "systems"]);
    }

    #[test]
    fn empty_tag_value_treated_as_text() {
        // `tag:` followed by space doesn't match tag pattern, becomes plain text
        let parsed = SearchQuery::from("tag: hello");
        assert_eq!(parsed.text, "tag: hello");
        assert!(parsed.tags.is_empty());
    }

    #[test]
    fn empty_folder_value_treated_as_text() {
        // `folder:` followed by space doesn't match folder pattern, becomes plain text
        let parsed = SearchQuery::from("folder: hello");
        assert_eq!(parsed.text, "folder: hello");
        assert!(parsed.folders.is_empty());
    }

    #[test]
    fn multiple_folders() {
        let parsed = SearchQuery::from("folder:a/ folder:b/");
        assert_eq!(parsed.folders.len(), 2);
    }

    // --- Quoted string tests ---

    #[test]
    fn quoted_tag_value() {
        let parsed = SearchQuery::from(r#"tag:"multi word" search"#);
        assert_eq!(parsed.tags, vec!["multi word"]);
        assert_eq!(parsed.text, "search");
    }

    #[test]
    fn quoted_folder_value() {
        let parsed = SearchQuery::from(r#"folder:"my folder/" search"#);
        assert_eq!(parsed.folders.len(), 1);
        assert_eq!(parsed.folders[0].to_string(), "my folder/");
        assert_eq!(parsed.text, "search");
    }

    #[test]
    fn unclosed_quote_treats_rest_as_token() {
        let parsed = SearchQuery::from(r#"tag:"open ended"#);
        assert_eq!(parsed.tags, vec!["open ended"]);
    }

    #[test]
    fn mixed_quoted_and_unquoted() {
        let parsed =
            SearchQuery::from(r#"event sourcing tag:"project management" folder:concepts/"#);
        assert_eq!(parsed.text, "event sourcing");
        assert_eq!(parsed.tags, vec!["project management"]);
        assert_eq!(parsed.folders.len(), 1);
    }

    #[test]
    fn quoted_text_not_a_filter() {
        let parsed = SearchQuery::from(r#""hello world" search"#);
        assert_eq!(parsed.text, "hello world search");
    }

    #[test]
    fn deserialize_from_json_string() {
        let json = r#""rust tag:programming""#;
        let query: SearchQuery = serde_json::from_str(json).unwrap();
        assert_eq!(query.text, "rust");
        assert_eq!(query.tags, vec!["programming"]);
    }
}
