//! Markdown note parsing and content extraction.
//!
//! This module provides types for parsing Obsidian-compatible Markdown notes,
//! extracting structured data like frontmatter, sections, links, and tags.
//!
//! ## Core Types
//!
//! - [`Note`] - Parsed representation of a Markdown note
//! - [`Section`] - Content block under a heading (or root content)
//! - [`Frontmatter`] - YAML frontmatter metadata
//! - [`Link`] - Parsed link (wiki, Markdown, URL, or email)
//! - [`Tag`] - Inline tag extracted from Markdown
//!
//! ## Extraction Trait
//!
//! The [`ExtractFrom`] trait provides a unified interface for extracting
//! parsed items from Markdown text:
//!
//! ```
//! use tarn::note_handler::{ExtractFrom, Tag, Link, Section};
//!
//! let text = "Some #tag and [[link]] content.";
//! let tags = Tag::extract_from(text);
//! let links = Link::extract_from(text);
//! ```
//!
//! ## Parsing
//!
//! Notes are parsed from Markdown strings with automatic detection of:
//!
//! - **Frontmatter**: YAML block between `---` delimiters
//! - **Sections**: Content split by headings, preserving hierarchy
//! - **Tags**: Both inline (`#tag`) and frontmatter (`tags: [a, b]`)
//! - **Links**: Wiki links (`[[target]]`), Markdown links, URLs, emails
//!
//! ## Example
//!
//! ```
//! use tarn::note_handler::Note;
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

mod error;
mod frontmatter;
mod links;
mod note;
mod sections;
mod tags;
mod tasks;

pub use error::NoteHandlerError;
pub use frontmatter::{Frontmatter, FrontmatterValue};
pub use links::{EmailLink, Link, MarkdownLink, UrlLink, WikiLink};
pub use note::Note;
pub use sections::{Heading, Section};
pub use tags::Tag;
pub use tasks::{Task, TaskStatus};

/// Trait for extracting parsed items from Markdown text.
///
/// This trait provides a unified interface for extracting structured data
/// (tags, links, sections, tasks) from Markdown content. Each implementor
/// defines its own output collection type.
///
/// # Example
///
/// ```
/// use tarn::note_handler::{ExtractFrom, Tag, Link};
///
/// let text = "Check #todo and see [[notes/reference]].";
/// let tags = Tag::extract_from(text);
/// let links = Link::extract_from(text);
/// ```
pub trait ExtractFrom: Sized {
    /// Output collection type (e.g., `HashSet<Self>`, `Vec<Self>`).
    type Output;

    /// Extract structured data from the given text.
    fn extract_from(text: &str) -> Self::Output;
}
