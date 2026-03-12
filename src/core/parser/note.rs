use std::collections::HashSet;
use std::fmt;
use std::path::PathBuf;

use thiserror::Error;

use super::frontmatter::Frontmatter;
use super::links::Link;
use super::sections::{Heading, Section, parse_sections};

use super::frontmatter::{split_frontmatter, try_split_frontmatter};

#[derive(Debug, Error)]
pub enum ParseNoteError {
    #[error("invalid frontmatter YAML: {0}")]
    InvalidFrontmatter(#[from] yaml_serde::Error),
}

/// Result of parsing an Obsidian markdown note.
#[derive(Debug, Clone)]
pub struct Note {
    pub path: Option<PathBuf>,
    pub title: Option<String>,
    pub frontmatter: Frontmatter,
    pub sections: Vec<Section>,
}

impl Note {
    /// Parse a note, propagating frontmatter errors.
    pub fn try_parse(content: &str) -> Result<Self, ParseNoteError> {
        let (frontmatter, body) = try_split_frontmatter(content)?;
        let sections = parse_sections(&body);
        let title = derive_title(&frontmatter, &sections);

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
            .flat_map(|s| s.tags.iter().map(String::as_str))
            .chain(self.frontmatter_tags())
            .collect()
    }

    pub fn word_count(&self) -> usize {
        self.sections.iter().map(|s| s.word_count).sum()
    }

    fn frontmatter_tags(&self) -> impl Iterator<Item = &str> {
        self.frontmatter.tags.iter().map(String::as_str)
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
        let sections = parse_sections(&body);
        let title = derive_title(&frontmatter, &sections);

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
        if self.frontmatter.title.is_some()
            || self.frontmatter.description.is_some()
            || !self.frontmatter.tags.is_empty()
            || !self.frontmatter.custom.is_empty()
        {
            writeln!(f, "---")?;
            let yaml = yaml_serde::to_string(&self.frontmatter).map_err(|_| fmt::Error)?;
            write!(f, "{yaml}")?;
            writeln!(f, "---")?;
        }
        for section in &self.sections {
            if let Some(h) = &section.heading {
                for _ in 0..h.level {
                    write!(f, "#")?;
                }
                writeln!(f, " {}", h.text)?;
            }
            write!(f, "{}", section.content)?;
        }
        Ok(())
    }
}

fn derive_title(frontmatter: &Frontmatter, sections: &[Section]) -> Option<String> {
    if let Some(t) = &frontmatter.title
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
    use crate::core::parser::frontmatter::FrontmatterValue;
    use crate::core::parser::links::WikiLink;

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

        assert_eq!(note.title.as_deref(), Some("My Note"));
        assert_eq!(
            note.frontmatter.custom.get("draft"),
            Some(&FrontmatterValue::Bool(true))
        );
        assert_eq!(
            note.frontmatter.custom.get("count"),
            Some(&FrontmatterValue::Number(42.0))
        );

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
        assert_eq!(note.frontmatter, Frontmatter::default());
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
        assert_eq!(note.frontmatter, Frontmatter::default());
        assert_eq!(note.sections.len(), 1);
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
        let content = "---\n: : invalid yaml [\n---\nBody.\n";
        let note = Note::from(content);
        assert_eq!(note.frontmatter, Frontmatter::default());
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

        // Re-parse the output
        let note2 = Note::from(output.as_str());
        assert_eq!(note2.title, note.title);
        assert_eq!(note2.frontmatter.tags, note.frontmatter.tags);
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
        let output = note.to_string();

        assert_eq!(output, content);
    }
}
