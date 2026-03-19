use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;

use super::ExtractFrom;
use super::links::Link;
use super::tags::Tag;
use super::tasks::Task;

/// Matches a heading line: captures (level hashes, heading text).
static HEADING_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^(#{1,6})\s+(.+?)(?:\s+#+)?$").expect("valid heading regex"));

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Heading {
    pub level: u8,
    pub text: String,
}

/// A section of a note delimited by headings.
/// The first section may have `heading: None` (content before any heading).
#[derive(Debug, Clone, PartialEq)]
pub struct Section {
    pub heading: Option<Heading>,
    /// Full heading path from root to this section.
    /// Example: `["Project Alpha", "Goals", "Q1"]` for a `## Q1` heading
    /// under `## Goals` under `# Project Alpha`.
    pub heading_path: Vec<String>,
    pub content: String,
    pub links: HashSet<Link>,
    pub tags: HashSet<Tag>,
    pub tasks: Vec<Task>,
}

impl Section {
    /// Count of words in the section content.
    pub fn word_count(&self) -> usize {
        self.content.split_whitespace().count()
    }
}

impl ExtractFrom for Heading {
    type Output = Option<Heading>;

    /// Extract a heading from the first line of text if it matches.
    fn extract_from(text: &str) -> Self::Output {
        let first_line = text.lines().next()?;
        let caps = HEADING_RE.captures(first_line)?;
        Some(Heading {
            level: caps[1].len() as u8,
            text: caps[2].to_string(),
        })
    }
}

impl ExtractFrom for Section {
    type Output = Vec<Section>;

    fn extract_from(text: &str) -> Self::Output {
        let mut sections = Vec::new();
        let mut last = 0;

        for m in HEADING_RE.find_iter(text) {
            sections.push(&text[last..m.start()]);
            last = m.start();
        }
        sections.push(&text[last..]);

        let mut heading_stack: Vec<(u8, String)> = Vec::new();
        sections
            .iter()
            .filter(|s| !s.trim().is_empty())
            .map(|&text_section| {
                let heading = Heading::extract_from(text_section);
                let heading_path = if let Some(ref h) = heading {
                    while heading_stack
                        .last()
                        .is_some_and(|(level, _)| *level >= h.level)
                    {
                        heading_stack.pop();
                    }
                    heading_stack.push((h.level, h.text.clone()));
                    heading_stack.iter().map(|(_, t)| t.clone()).collect()
                } else {
                    Vec::new()
                };

                Section {
                    heading,
                    heading_path,
                    content: text_section.to_string(),
                    links: Link::extract_from(text_section),
                    tags: Tag::extract_from(text_section),
                    tasks: Task::extract_from(text_section),
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_from_by_headings() {
        let body = "\
Intro paragraph.

## Section One

Content one.

### Subsection

Nested content.

## Section Two

Content two.
";
        let sections = Section::extract_from(body);

        assert_eq!(sections.len(), 4);

        // First section: no heading (intro)
        assert!(sections[0].heading.is_none());
        assert!(sections[0].content.contains("Intro paragraph"));

        // Second section: ## Section One
        let h1 = sections[1].heading.as_ref().unwrap();
        assert_eq!(h1.level, 2);
        assert_eq!(h1.text, "Section One");

        // Third section: ### Subsection
        let h2 = sections[2].heading.as_ref().unwrap();
        assert_eq!(h2.level, 3);
        assert_eq!(h2.text, "Subsection");

        // Fourth section: ## Section Two
        let h3 = sections[3].heading.as_ref().unwrap();
        assert_eq!(h3.level, 2);
        assert_eq!(h3.text, "Section Two");
    }

    #[test]
    fn crlf_line_endings() {
        let body = "Intro\r\n\r\n## First\r\n\r\nContent\r\n\r\n## Second\r\n";
        let sections = Section::extract_from(body);
        let headings: Vec<_> = sections.iter().filter_map(|s| s.heading.as_ref()).collect();

        assert_eq!(headings.len(), 2);
        assert_eq!(headings[0].text, "First");
        assert_eq!(headings[1].text, "Second");
    }

    #[test]
    fn empty_content_produces_no_sections() {
        let sections = Section::extract_from("");
        assert!(sections.is_empty());
    }

    #[test]
    fn content_without_headings_single_section() {
        let sections = Section::extract_from("Just plain text.\n\nAnother paragraph.\n");
        assert_eq!(sections.len(), 1);
        assert!(sections[0].heading.is_none());
        assert!(sections[0].heading_path.is_empty());
        assert!(sections[0].content.contains("Just plain text"));
    }

    #[test]
    fn section_word_count() {
        let sections = Section::extract_from("# Title\n\nOne two three four five.\n");
        assert_eq!(sections.len(), 1);
        // "# Title", "", "One two three four five." — heading line + body
        assert!(sections[0].word_count() > 0);
        // "# Title One two three four five." = 7 words (split_whitespace)
        assert_eq!(sections[0].word_count(), 7);
    }

    #[test]
    fn heading_extract_from_non_heading() {
        assert!(Heading::extract_from("Just a paragraph.").is_none());
        assert!(Heading::extract_from("").is_none());
        assert!(Heading::extract_from("##nospace").is_none());
    }

    #[test]
    fn heading_with_atx_closing_hashes() {
        let heading = Heading::extract_from("## Title ##").unwrap();
        assert_eq!(heading.level, 2);
        assert_eq!(heading.text, "Title");
    }

    #[test]
    fn sections_contain_links_tags_tasks() {
        let body = "## Section\n\nSee [[target]] and #mytag here.\n\n- [ ] A task\n";
        let sections = Section::extract_from(body);
        assert_eq!(sections.len(), 1);
        assert!(!sections[0].links.is_empty());
        assert!(!sections[0].tags.is_empty());
        assert!(!sections[0].tasks.is_empty());
    }

    #[test]
    fn heading_path_builds_hierarchy() {
        let body = "\
Intro.

# Project Alpha

Overview.

## Goals

Goal content.

### Q1

Q1 targets.

### Q2

Q2 targets.

## Status

Current status.

# Project Beta

Another project.
";
        let sections = Section::extract_from(body);

        // Intro section: no heading, empty path
        assert!(sections[0].heading.is_none());
        assert!(sections[0].heading_path.is_empty());

        // # Project Alpha
        assert_eq!(sections[1].heading_path, vec!["Project Alpha"]);

        // ## Goals (under Project Alpha)
        assert_eq!(sections[2].heading_path, vec!["Project Alpha", "Goals"]);

        // ### Q1 (under Goals, under Project Alpha)
        assert_eq!(
            sections[3].heading_path,
            vec!["Project Alpha", "Goals", "Q1"]
        );

        // ### Q2 (sibling of Q1, under Goals)
        assert_eq!(
            sections[4].heading_path,
            vec!["Project Alpha", "Goals", "Q2"]
        );

        // ## Status (sibling of Goals, under Project Alpha)
        assert_eq!(sections[5].heading_path, vec!["Project Alpha", "Status"]);

        // # Project Beta (new top-level)
        assert_eq!(sections[6].heading_path, vec!["Project Beta"]);
    }
}
