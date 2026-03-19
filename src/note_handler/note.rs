use std::collections::HashSet;
use std::fmt;

use crate::common::VaultPath;

use super::ExtractFrom;
use super::error::NoteHandlerError;
use super::frontmatter::Frontmatter;
use super::links::Link;
use super::sections::{Heading, Section};

use super::frontmatter::{split_frontmatter, try_split_frontmatter};

/// Result of parsing an Obsidian Markdown note.
#[derive(Debug, Clone)]
pub struct Note {
    pub path: Option<VaultPath>,
    pub title: Option<String>,
    pub frontmatter: Option<Frontmatter>,
    pub sections: Vec<Section>,
}

impl Note {
    /// Parse a note, propagating frontmatter errors.
    pub fn try_parse(content: &str) -> Result<Self, NoteHandlerError> {
        let (frontmatter, body) = try_split_frontmatter(content)
            .map_err(|e| NoteHandlerError::InvalidFrontmatter(e.to_string()))?;
        let sections = Section::extract_from(&body);
        let title = derive_title(frontmatter.as_ref(), &sections);

        Ok(Note {
            path: None,
            title,
            frontmatter,
            sections,
        })
    }

    pub fn headings(&self) -> Vec<&Heading> {
        self.sections
            .iter()
            .filter_map(|s| s.heading.as_ref())
            .collect()
    }

    pub fn links(&self) -> Vec<&Link> {
        self.sections.iter().flat_map(|s| &s.links).collect()
    }

    pub fn tags(&self) -> HashSet<&str> {
        self.sections
            .iter()
            .flat_map(|s| s.tags.iter().map(|t| t.name()))
            .chain(self.frontmatter_tags())
            .collect()
    }

    pub fn word_count(&self) -> usize {
        self.sections.iter().map(|s| s.word_count()).sum()
    }

    fn frontmatter_tags(&self) -> impl Iterator<Item = &str> {
        self.frontmatter
            .as_ref()
            .map(|fm| fm.tags.as_slice())
            .unwrap_or(&[])
            .iter()
            .map(String::as_str)
    }
}

impl From<String> for Note {
    fn from(content: String) -> Self {
        Note::from(content.as_str())
    }
}

impl From<&str> for Note {
    fn from(content: &str) -> Self {
        let (frontmatter, body) = split_frontmatter(content);
        let sections = Section::extract_from(&body);
        let title = derive_title(frontmatter.as_ref(), &sections);

        Note {
            path: None,
            title,
            frontmatter,
            sections,
        }
    }
}

impl fmt::Display for Note {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(fm) = &self.frontmatter {
            writeln!(f, "---")?;
            let yaml = yaml_serde::to_string(fm).map_err(|_| fmt::Error)?;
            write!(f, "{yaml}")?;
            writeln!(f, "---")?;
        }
        for section in &self.sections {
            write!(f, "{}", section.content)?;
        }
        Ok(())
    }
}

fn derive_title(frontmatter: Option<&Frontmatter>, sections: &[Section]) -> Option<String> {
    if let Some(fm) = frontmatter
        && let Some(t) = &fm.title
        && !t.is_empty()
    {
        return Some(t.clone());
    }

    sections
        .iter()
        .find_map(|s| s.heading.as_ref())
        .filter(|h| h.level == 1)
        .map(|h| h.text.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::note_handler::frontmatter::FrontmatterValue;
    use crate::note_handler::links::WikiLink;
    use crate::note_handler::tags::Tag;

    #[test]
    fn parse_note_with_frontmatter() {
        let content = "\
---
title: My Note
tags: [rust, obsidian]
draft: true
count: 42
---
# My Note

Some content with a [[link]] and a #tag here.
";

        let note = Note::from(content);
        let fm = note.frontmatter.as_ref().expect("should have frontmatter");

        assert_eq!(note.title.as_deref(), Some("My Note"));
        assert_eq!(fm.custom.get("draft"), Some(&FrontmatterValue::Bool(true)));
        assert_eq!(fm.custom.get("count"), Some(&FrontmatterValue::Int(42)));

        // Tags from frontmatter
        let tags = note.tags();
        assert!(tags.contains("rust"));
        assert!(tags.contains("obsidian"));
        // Inline tag
        assert!(tags.contains("tag"));

        // Links
        let links = note.links();
        assert_eq!(links.len(), 1);
        assert!(matches!(links[0], Link::Wiki(WikiLink { target, .. }) if target == "link"));

        assert!(note.word_count() > 0);
    }

    #[test]
    fn parse_note_without_frontmatter() {
        let content = "# Hello World\n\nSome paragraph text.\n";
        let note = Note::from(content);

        assert_eq!(note.title.as_deref(), Some("Hello World"));
        assert!(note.frontmatter.is_none());
    }

    #[test]
    fn frontmatter_list_with_dashes() {
        let content = "\
---
tags:
  - alpha
  - beta
  - gamma
---
Body text.
";
        let note = Note::from(content);
        let tags = note.tags();

        assert!(tags.contains("alpha"));
        assert!(tags.contains("beta"));
        assert!(tags.contains("gamma"));
    }

    #[test]
    fn title_falls_back_to_h1() {
        let content = "\
---
author: Someone
---
# The Real Title

Content.
";
        let note = Note::from(content);
        assert_eq!(note.title.as_deref(), Some("The Real Title"));
    }

    #[test]
    fn nested_tags() {
        let content = "Some text with #project/alpha and #project/beta tags.\n";
        let note = Note::from(content);
        let tags = note.tags();

        assert!(tags.contains("project/alpha"));
        assert!(tags.contains("project/beta"));
    }

    #[test]
    fn empty_content() {
        let note = Note::from("");
        assert_eq!(note.title, None);
        assert!(note.frontmatter.is_none());
        assert_eq!(note.sections.len(), 0); // No sections for empty content
        assert_eq!(note.word_count(), 0);
    }

    #[test]
    fn try_parse_propagates_frontmatter_error() {
        let content = "---\n: : invalid yaml [\n---\nBody.\n";
        let result = Note::try_parse(content);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("frontmatter"));
    }

    #[test]
    fn malformed_frontmatter_falls_back_to_default() {
        // When From<&str> encounters invalid YAML, it falls back to default Frontmatter
        let content = "---\n: : invalid yaml [\n---\nBody.\n";
        let note = Note::from(content);
        let fm = note
            .frontmatter
            .as_ref()
            .expect("malformed YAML yields default frontmatter");
        assert_eq!(fm.title, None);
        assert!(fm.tags.is_empty());
        assert!(fm.aliases.is_empty());
        assert!(fm.custom.is_empty());
        assert_eq!(note.sections[0].content, "Body.\n");
    }

    #[test]
    fn note_display_roundtrip() {
        let content =
            "---\ntitle: Test\ntags:\n  - rust\n  - obsidian\n---\n# Test\n\nBody content.\n";
        let note = Note::from(content);
        let output = note.to_string();

        assert!(output.contains("---"));
        assert!(output.contains("title"));
        assert!(output.contains("Body content."));

        // Reparse the output
        let note2 = Note::from(output.as_str());
        assert_eq!(note2.title, note.title);
        assert_eq!(
            note2.frontmatter.as_ref().unwrap().tags,
            note.frontmatter.as_ref().unwrap().tags
        );
    }

    #[test]
    fn parse_and_reconstruct_obsidian_note() {
        let content = "\
---
title: Rust Ownership
description: Notes on Rust's ownership model
tags:
- rust
- programming
- memory-safety
draft: true
---
# Rust Ownership

Every value in Rust has a single **owner**. See [[The Rust Book]] for details.

## Borrowing

References allow #borrowing without taking ownership.

- Shared references: `&T`
- Mutable references: `&mut T`

Check [the docs](https://doc.rust-lang.org/book/ch04-02-references-and-borrowing.html \"Borrowing\") for more.

## Lifetimes

Lifetimes ensure references are valid. Related: [[Lifetime Elision|elision rules]].

```rust
fn longest<'a>(x: &'a str, y: &'a str) -> &'a str {
    if x.len() > y.len() { x } else { y }
}
```

#rust/advanced #lifetime

## Summary

See also:
- [[Smart Pointers]]
- [[Concurrency#Shared State]]
- ![[ownership-diagram.png]]
- <https://doc.rust-lang.org/nomicon/>
- <user@rust-lang.org>
";

        let note = Note::from(content);

        // Verify section structure:
        // 4 sections total (no empty root section when content starts with heading)
        assert_eq!(note.sections.len(), 4);

        // Section 0: # Rust Ownership
        assert_eq!(note.sections[0].heading.as_ref().unwrap().level, 1);
        assert_eq!(
            note.sections[0].heading.as_ref().unwrap().text,
            "Rust Ownership"
        );
        assert_eq!(note.sections[0].heading_path, vec!["Rust Ownership"]);
        assert!(note.sections[0].content.contains("Every value in Rust"));

        // Section 1: ## Borrowing
        assert_eq!(note.sections[1].heading.as_ref().unwrap().level, 2);
        assert_eq!(note.sections[1].heading.as_ref().unwrap().text, "Borrowing");
        assert_eq!(
            note.sections[1].heading_path,
            vec!["Rust Ownership", "Borrowing"]
        );
        assert!(
            note.sections[1]
                .tags
                .contains(&Tag::new("#borrowing").unwrap())
        );

        // Section 2: ## Lifetimes
        assert_eq!(note.sections[2].heading.as_ref().unwrap().level, 2);
        assert_eq!(note.sections[2].heading.as_ref().unwrap().text, "Lifetimes");
        assert_eq!(
            note.sections[2].heading_path,
            vec!["Rust Ownership", "Lifetimes"]
        );
        assert!(
            note.sections[2]
                .tags
                .contains(&Tag::new("#rust/advanced").unwrap())
        );
        assert!(
            note.sections[2]
                .tags
                .contains(&Tag::new("#lifetime").unwrap())
        );

        // Section 3: ## Summary
        assert_eq!(note.sections[3].heading.as_ref().unwrap().level, 2);
        assert_eq!(note.sections[3].heading.as_ref().unwrap().text, "Summary");
        assert_eq!(
            note.sections[3].heading_path,
            vec!["Rust Ownership", "Summary"]
        );

        // Verify output contains expected content (exact equality not possible due to created/modified timestamps)
        let output = note.to_string();
        assert!(output.contains("title: Rust Ownership"));
        assert!(output.contains("draft: true"));
        assert!(output.contains("# Rust Ownership"));
        assert!(output.contains("[[The Rust Book]]"));

        // Verify roundtrip: reparsing produces same semantic content
        let note2 = Note::from(output.as_str());
        assert_eq!(
            note2.frontmatter.as_ref().unwrap().title,
            note.frontmatter.as_ref().unwrap().title
        );
        assert_eq!(
            note2.frontmatter.as_ref().unwrap().tags,
            note.frontmatter.as_ref().unwrap().tags
        );
        assert_eq!(note2.sections.len(), note.sections.len());
    }

    #[test]
    fn parses_frontmatter_with_crlf_line_endings() {
        // Windows-style CRLF line endings
        let content = "---\r\ntitle: Test\r\ntags:\r\n  - rust\r\n  - windows\r\n---\r\n# Test\r\n\r\nBody content.\r\n";
        let note = Note::from(content);
        let fm = note.frontmatter.as_ref().expect("should have frontmatter");

        assert_eq!(fm.title, Some("Test".to_string()));
        assert_eq!(fm.tags, vec!["rust", "windows"]);
        assert!(!note.sections.is_empty());
    }

    #[test]
    fn try_parse_success() {
        // Test Note::try_parse with valid content returns Ok(Note)
        let content = "---\ntitle: Valid Note\ntags:\n  - test\n---\n# Heading\n\nBody content.";
        let result = Note::try_parse(content);

        assert!(result.is_ok());
        let note = result.unwrap();
        let fm = note.frontmatter.as_ref().expect("should have frontmatter");
        assert_eq!(note.title, Some("Valid Note".to_string()));
        assert_eq!(fm.tags, vec!["test"]);
        assert!(!note.sections.is_empty());
    }

    #[test]
    fn note_headings_returns_all_headings() {
        // Test headings() method extracts all section headings
        let content = "\
# First Heading

Content.

## Second Heading

More content.

### Third Heading

Even more content.
";
        let note = Note::from(content);
        let headings = note.headings();

        assert_eq!(headings.len(), 3);
        assert_eq!(headings[0].level, 1);
        assert_eq!(headings[0].text, "First Heading");
        assert_eq!(headings[1].level, 2);
        assert_eq!(headings[1].text, "Second Heading");
        assert_eq!(headings[2].level, 3);
        assert_eq!(headings[2].text, "Third Heading");
    }

    #[test]
    fn try_parse_error_variant_is_invalid_frontmatter() {
        let content = "---\n: : invalid yaml [\n---\nBody.\n";
        let err = Note::try_parse(content).unwrap_err();
        assert!(matches!(err, NoteHandlerError::InvalidFrontmatter(_)));
    }

    #[test]
    fn try_parse_no_frontmatter_succeeds() {
        let content = "Just plain text, no frontmatter delimiters.";
        let note = Note::try_parse(content).unwrap();
        assert!(note.frontmatter.is_none());
        assert!(!note.sections.is_empty());
    }

    #[test]
    fn note_with_frontmatter_empty_body() {
        let content = "---\ntitle: Empty Body\n---\n";
        let note = Note::from(content);
        assert_eq!(note.title.as_deref(), Some("Empty Body"));
        assert!(note.frontmatter.is_some());
        assert_eq!(note.sections.len(), 0);
        assert_eq!(note.word_count(), 0);
    }

    #[test]
    fn note_from_string_owned() {
        // Test From<String> impl works same as From<&str>
        let content = String::from("# Test\n\nSome content.");
        let note_from_string = Note::from(content.clone());
        let note_from_str = Note::from(content.as_str());

        assert_eq!(note_from_string.title, note_from_str.title);
        assert_eq!(
            note_from_string.sections.len(),
            note_from_str.sections.len()
        );
        assert_eq!(note_from_string.frontmatter, note_from_str.frontmatter);
    }
}
