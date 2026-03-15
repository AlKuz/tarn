//! Markdown note parsing and content extraction.
//!
//! This module provides types for parsing Obsidian-compatible markdown notes,
//! extracting structured data like frontmatter, sections, links, and tags.
//!
//! ## Core Types
//!
//! - [`Note`] - Parsed representation of a markdown note
//! - [`Section`] - Content block under a heading (or root content)
//! - [`Frontmatter`] - YAML frontmatter metadata
//! - [`Link`] - Parsed link (wiki, markdown, URL, or email)
//!
//! ## Parsing
//!
//! Notes are parsed from markdown strings with automatic detection of:
//!
//! - **Frontmatter**: YAML block between `---` delimiters
//! - **Sections**: Content split by headings, preserving hierarchy
//! - **Tags**: Both inline (`#tag`) and frontmatter (`tags: [a, b]`)
//! - **Links**: Wiki links (`[[target]]`), markdown links, URLs, emails
//!
//! ## Example
//!
//! ```
//! use tarn::note::Note;
//!
//! let content = r#"---
//! title: Example
//! tags: [rust, example]
//! ---
//! # Introduction
//!
//! See [[other-note]] for details.
//!
//! ## Subsection
//!
//! More content here.
//! "#;
//!
//! let note = Note::from(content);
//! assert_eq!(note.title, Some("Example".to_string()));
//! assert_eq!(note.sections.len(), 2);
//! ```

pub mod frontmatter;
pub mod links;
mod parser;
mod sections;
mod tags;

pub use frontmatter::{Frontmatter, FrontmatterValue};
pub use links::{EmailLink, Link, MarkdownLink, ParseLinkError, UrlLink, WikiLink};
pub use parser::{Note, ParseNoteError};
pub use sections::{Heading, Section};
