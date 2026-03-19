use std::collections::HashSet;
use std::fmt::Display;
use std::str::FromStr;
use std::sync::LazyLock;

use regex::Regex;

use super::ExtractFrom;
use super::error::NoteHandlerError;

/// Validates that a string is a properly formatted tag.
static TAG_VALIDATION_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^#[\w/_-]+$").expect("valid tag validation regex"));

/// Extracts tag candidates from text (captures the name without #).
static TAG_CANDIDATE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"#([\w/_-]+)").expect("valid tag candidate regex"));

/// Matches fenced code blocks and inline code spans.
static CODE_BLOCK_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"```[\s\S]*?```|``[^`]+``|`[^`]+`").expect("valid code block regex")
});

/// Matches wikilinks including heading references.
static WIKILINK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[\[[^]]+]]").expect("valid wikilink regex"));

/// An inline tag extracted from Markdown content.
///
/// Tags start with `#` followed by alphanumeric characters, underscores,
/// hyphens, or slashes (for nested tags like `#project/alpha`).
///
/// Pure numeric tags (like `#123`) are excluded as they typically represent
/// issue references rather than categorization tags.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Tag(String);

impl Tag {
    /// Create a new tag from a string, validating the format.
    ///
    /// The string must start with `#` followed by valid tag characters.
    ///
    /// # Errors
    ///
    /// Returns `NoteHandlerError::InvalidTag` if:
    /// - The tag doesn't start with `#`
    /// - The tag contains invalid characters (only alphanumeric, `_`, `-`, `/` allowed after `#`)
    /// - The tag is purely numeric (e.g., "#123")
    pub fn new(tag: impl Into<String>) -> Result<Self, NoteHandlerError> {
        let tag = tag.into();
        Self::validate(&tag)?;
        Ok(Self(tag))
    }

    fn validate(tag: &str) -> Result<(), NoteHandlerError> {
        // Validate format: #[a-zA-Z0-9/_-]+
        if !TAG_VALIDATION_RE.is_match(tag) {
            return Err(NoteHandlerError::InvalidTag {
                tag: tag.to_string(),
            });
        }

        // Exclude purely numeric tags (e.g., #123)
        let name = &tag[1..];
        if name.chars().all(|c| c.is_ascii_digit()) {
            return Err(NoteHandlerError::InvalidTag {
                tag: tag.to_string(),
            });
        }

        Ok(())
    }

    /// Get the tag value as a string slice (includes the `#` prefix).
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Get the tag name without the `#` prefix.
    pub fn name(&self) -> &str {
        &self.0[1..]
    }
}

impl Display for Tag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl AsRef<str> for Tag {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<Tag> for String {
    fn from(tag: Tag) -> Self {
        tag.0
    }
}

impl FromStr for Tag {
    type Err = NoteHandlerError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl ExtractFrom for Tag {
    type Output = HashSet<Tag>;

    fn extract_from(text: &str) -> Self::Output {
        // Remove code blocks and inline code spans
        let text = CODE_BLOCK_RE.replace_all(text, "");
        // Remove wikilinks (which may contain # for heading references)
        let text = WIKILINK_RE.replace_all(&text, "");

        let mut tags = HashSet::new();
        for caps in TAG_CANDIDATE_RE.captures_iter(&text) {
            let candidate = format!("#{}", &caps[1]);
            // Validate and insert; skip invalid tags (e.g., #123)
            if let Ok(tag) = Tag::new(candidate) {
                tags.insert(tag);
            }
        }

        tags
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_simple_tags() {
        let content = "Some #tag here and #another there.";
        let tags = Tag::extract_from(content);

        assert_eq!(tags.len(), 2);
        assert!(tags.contains(&Tag::new("#tag").unwrap()));
        assert!(tags.contains(&Tag::new("#another").unwrap()));
    }

    #[test]
    fn extract_nested_tags() {
        let content = "Working on #project/alpha and #project/beta.";
        let tags = Tag::extract_from(content);

        assert_eq!(tags.len(), 2);
        assert!(tags.contains(&Tag::new("#project/alpha").unwrap()));
        assert!(tags.contains(&Tag::new("#project/beta").unwrap()));
    }

    #[test]
    fn extract_tags_with_numbers() {
        let content = "Version #v2 and #release-3 but not #123.";
        let tags = Tag::extract_from(content);

        assert_eq!(tags.len(), 2);
        assert!(tags.contains(&Tag::new("#v2").unwrap()));
        assert!(tags.contains(&Tag::new("#release-3").unwrap()));
        // Pure numeric tags are excluded
        assert!(!tags.iter().any(|t| t.name() == "123"));
    }

    #[test]
    fn extract_tags_at_line_boundaries() {
        let content = "#start of line\nand end #end\n#solo";
        let tags = Tag::extract_from(content);

        assert_eq!(tags.len(), 3);
        assert!(tags.contains(&Tag::new("#start").unwrap()));
        assert!(tags.contains(&Tag::new("#end").unwrap()));
        assert!(tags.contains(&Tag::new("#solo").unwrap()));
    }

    #[test]
    fn extract_multiple_tags_per_line() {
        let content = "#one #two #three on same line";
        let tags = Tag::extract_from(content);

        assert_eq!(tags.len(), 3);
        assert!(tags.contains(&Tag::new("#one").unwrap()));
        assert!(tags.contains(&Tag::new("#two").unwrap()));
        assert!(tags.contains(&Tag::new("#three").unwrap()));
    }

    #[test]
    fn extract_tags_adjacent_to_punctuation() {
        let content = "A sentence with #tag,#another, here.";
        let tags = Tag::extract_from(content);

        assert_eq!(tags.len(), 2);
        assert!(tags.contains(&Tag::new("#tag").unwrap()));
        assert!(tags.contains(&Tag::new("#another").unwrap()));
    }

    #[test]
    fn extract_tags_with_underscores_and_hyphens() {
        let content = "Tags like #under_score and #kebab-case work.";
        let tags = Tag::extract_from(content);

        assert_eq!(tags.len(), 2);
        assert!(tags.contains(&Tag::new("#under_score").unwrap()));
        assert!(tags.contains(&Tag::new("#kebab-case").unwrap()));
    }

    #[test]
    fn extract_empty_content() {
        let content = "";
        let tags = Tag::extract_from(content);

        assert!(tags.is_empty());
    }

    #[test]
    fn extract_content_without_tags() {
        let content = "No tags in this content at all.";
        let tags = Tag::extract_from(content);

        assert!(tags.is_empty());
    }

    #[test]
    fn tags_not_extracted_from_code_blocks() {
        let content = "\
Real #tag here.

```
#not-a-tag inside fence
```

Also `#not-inline-tag` in code span.
";
        let tags = Tag::extract_from(content);

        assert_eq!(tags.len(), 1);
        assert!(tags.contains(&Tag::new("#tag").unwrap()));
        assert!(!tags.iter().any(|t| t.name() == "not-a-tag"));
        assert!(!tags.iter().any(|t| t.name() == "not-inline-tag"));
    }

    #[test]
    fn tags_not_extracted_from_double_backtick_code() {
        let content = "Real #tag but ``#code-tag`` is ignored.";
        let tags = Tag::extract_from(content);

        assert_eq!(tags.len(), 1);
        assert!(tags.contains(&Tag::new("#tag").unwrap()));
    }

    #[test]
    fn duplicate_tags_are_deduplicated() {
        let content = "#same tag and #same again and #same once more";
        let tags = Tag::extract_from(content);

        assert_eq!(tags.len(), 1);
        assert!(tags.contains(&Tag::new("#same").unwrap()));
    }

    #[test]
    fn tag_display_and_as_ref() {
        let tag = Tag::new("#project/alpha").unwrap();

        assert_eq!(tag.to_string(), "#project/alpha");
        assert_eq!(tag.as_str(), "#project/alpha");
        assert_eq!(tag.as_ref(), "#project/alpha");
        assert_eq!(tag.name(), "project/alpha");
    }

    #[test]
    fn tag_into_string() {
        let tag = Tag::new("#example").unwrap();
        let s: String = tag.into();

        assert_eq!(s, "#example");
    }

    #[test]
    fn tag_new_validates_format() {
        // Valid tags (must start with #)
        assert!(Tag::new("#valid").is_ok());
        assert!(Tag::new("#with-hyphen").is_ok());
        assert!(Tag::new("#with_underscore").is_ok());
        assert!(Tag::new("#nested/tag").is_ok());
        assert!(Tag::new("#v2").is_ok());
        assert!(Tag::new("#2fast").is_ok());

        // Invalid tags
        assert!(Tag::new("").is_err()); // empty
        assert!(Tag::new("#").is_err()); // just hash
        assert!(Tag::new("#123").is_err()); // purely numeric
        assert!(Tag::new("no-hash").is_err()); // missing #
        assert!(Tag::new("#has space").is_err()); // contains space
        assert!(Tag::new("#has.dot").is_err()); // contains dot
        assert!(Tag::new("#has@symbol").is_err()); // contains @
    }

    #[test]
    fn tag_from_str() {
        let tag: Tag = "#valid-tag".parse().unwrap();
        assert_eq!(tag.as_str(), "#valid-tag");
        assert_eq!(tag.name(), "valid-tag");

        let result: Result<Tag, _> = "#123".parse();
        assert!(result.is_err());

        let result: Result<Tag, _> = "no-hash".parse();
        assert!(result.is_err());
    }

    #[test]
    fn unicode_tags_extracted() {
        let content = "A #café tag and #naïve too.";
        let tags = Tag::extract_from(content);
        assert!(tags.contains(&Tag::new("#café").unwrap()));
        assert!(tags.contains(&Tag::new("#naïve").unwrap()));
    }

    #[test]
    fn tag_at_eof_without_newline() {
        let content = "Some text #final";
        let tags = Tag::extract_from(content);
        assert!(tags.contains(&Tag::new("#final").unwrap()));
    }

    #[test]
    fn wikilink_headings_not_extracted_as_tags() {
        let content = "Link to [[#heading]] and [[note#section]] here.";
        let tags = Tag::extract_from(content);
        assert!(tags.is_empty());
    }
}
