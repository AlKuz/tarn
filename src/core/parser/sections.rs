use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;

use super::links::Link;
use super::tags::extract_inline_tags;

static HEADING_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^(#{1,6})\s+(.+?)(?:\s+#+)?$").expect("valid heading regex"));

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Heading {
    pub level: u8,
    pub text: String,
    pub offset: usize,
}

/// A section of a note delimited by headings.
/// The first section may have `heading: None` (content before any heading).
#[derive(Debug, Clone, PartialEq)]
pub struct Section {
    pub heading: Option<Heading>,
    pub content: String,
    pub links: Vec<Link>,
    pub tags: HashSet<String>,
    pub word_count: usize,
}

pub(crate) fn parse_sections(body: &str) -> Vec<Section> {
    let mut sections: Vec<Section> = Vec::new();
    let mut current_heading: Option<Heading> = None;
    let mut current_content = String::new();
    let mut offset = 0;

    for line in body.lines() {
        if let Some(caps) = HEADING_RE.captures(line) {
            let level = caps[1].len() as u8;
            let text = caps[2].to_string();
            let heading = Heading {
                level,
                text,
                offset,
            };

            // Flush previous section
            let section = build_section(current_heading.take(), &current_content);
            sections.push(section);
            current_heading = Some(heading);
            current_content.clear();
            offset += line.len() + line_ending_len(body, offset + line.len());
            continue;
        }

        current_content.push_str(line);
        current_content.push('\n');
        offset += line.len() + line_ending_len(body, offset + line.len());
    }

    // Flush last section
    let section = build_section(current_heading, &current_content);
    sections.push(section);

    sections
}

/// Determine the line ending length at the given byte position.
/// Handles `\r\n`, `\n`, and end-of-string.
fn line_ending_len(s: &str, pos: usize) -> usize {
    let bytes = s.as_bytes();
    if bytes.get(pos) == Some(&b'\r') && bytes.get(pos + 1) == Some(&b'\n') {
        2
    } else if bytes.get(pos) == Some(&b'\n') {
        1
    } else {
        0
    }
}

fn build_section(heading: Option<Heading>, content: &str) -> Section {
    let links = Link::extract(content);
    let tags = extract_inline_tags(content);
    let word_count = content.split_whitespace().count();

    Section {
        heading,
        content: content.to_string(),
        links,
        tags,
        word_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_sections_by_headings() {
        let body = "\
Intro paragraph.

## Section One

Content one.

### Subsection

Nested content.

## Section Two

Content two.
";
        let sections = parse_sections(body);

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
    fn heading_offsets_are_tracked() {
        let body = "Intro\n\n## First\n\nContent\n\n## Second\n";
        let sections = parse_sections(body);
        let headings: Vec<_> = sections.iter().filter_map(|s| s.heading.as_ref()).collect();

        assert_eq!(headings.len(), 2);
        assert_eq!(headings[0].text, "First");
        assert!(headings[0].offset > 0);
        assert!(headings[1].offset > headings[0].offset);
    }

    #[test]
    fn crlf_line_endings() {
        let body = "Intro\r\n\r\n## First\r\n\r\nContent\r\n\r\n## Second\r\n";
        let sections = parse_sections(body);
        let headings: Vec<_> = sections.iter().filter_map(|s| s.heading.as_ref()).collect();

        assert_eq!(headings.len(), 2);
        assert_eq!(headings[0].text, "First");
        assert_eq!(headings[1].text, "Second");
        assert!(headings[1].offset > headings[0].offset);
    }

}
