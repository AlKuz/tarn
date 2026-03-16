use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;

use super::links::Link;
use super::tags::extract_inline_tags;

static HEADING_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^(#{1,6})\s+(.+?)(?:\s+#+)?$").expect("valid heading regex"));

static TASK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^[\t ]*- \[(.)]\s*(.*)$").expect("valid task regex"));

/// A task item extracted from a note (checkbox list item).
#[derive(Debug, Clone, PartialEq)]
pub struct Task {
    /// 1-based line number where the task appears.
    pub line: usize,
    /// Task completion status.
    pub status: TaskStatus,
    /// Task description text (without the checkbox marker).
    pub text: String,
}

/// Completion status of a task checkbox.
#[derive(Debug, Clone, PartialEq)]
pub enum TaskStatus {
    /// Incomplete task: `- [ ]`
    Open,
    /// Completed task: `- [x]`
    Done,
    /// Cancelled task: `- [-]`
    Canceled,
    /// Custom status marker, e.g. `- [>]` for deferred.
    Custom(char),
}

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
    /// Full heading path from root to this section.
    /// Example: `["Project Alpha", "Goals", "Q1"]` for a `## Q1` heading
    /// under `## Goals` under `# Project Alpha`.
    pub heading_path: Vec<String>,
    pub content: String,
    pub links: Vec<Link>,
    pub tags: HashSet<String>,
    pub word_count: usize,
    pub tasks: Vec<Task>,
}

pub(crate) fn parse_sections(body: &str) -> Vec<Section> {
    let mut sections: Vec<Section> = Vec::new();
    let mut current_heading: Option<Heading> = None;
    let mut current_content = String::new();
    let mut offset = 0;
    let mut line_number = 1usize;
    let mut section_start_line = 1usize;
    // Stack of (level, text) for building heading paths
    let mut heading_stack: Vec<(u8, String)> = Vec::new();
    let mut current_heading_path: Vec<String> = Vec::new();

    for line in body.lines() {
        if let Some(caps) = HEADING_RE.captures(line) {
            let level = caps[1].len() as u8;
            let text = caps[2].to_string();
            let heading = Heading {
                level,
                text: text.clone(),
                offset,
            };

            // Flush previous section (only if there's content or a heading)
            if current_heading.is_some() || !current_content.trim().is_empty() {
                let section = build_section(
                    current_heading.take(),
                    current_heading_path.clone(),
                    &current_content,
                    section_start_line,
                );
                sections.push(section);
            }

            // Update heading stack: pop entries at same or deeper level
            while heading_stack
                .last()
                .is_some_and(|(stack_level, _)| *stack_level >= level)
            {
                heading_stack.pop();
            }
            heading_stack.push((level, text.clone()));

            // Build current heading path from stack
            current_heading_path = heading_stack.iter().map(|(_, t)| t.clone()).collect();

            current_heading = Some(heading);
            current_content.clear();
            section_start_line = line_number + 1;
            offset += line.len() + line_ending_len(body, offset + line.len());
            line_number += 1;
            continue;
        }

        current_content.push_str(line);
        current_content.push('\n');
        offset += line.len() + line_ending_len(body, offset + line.len());
        line_number += 1;
    }

    // Flush last section (only if there's content or a heading)
    if current_heading.is_some() || !current_content.trim().is_empty() {
        let section = build_section(
            current_heading,
            current_heading_path,
            &current_content,
            section_start_line,
        );
        sections.push(section);
    }

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

fn build_section(
    heading: Option<Heading>,
    heading_path: Vec<String>,
    content: &str,
    start_line: usize,
) -> Section {
    let links = Link::extract(content);
    let tags = extract_inline_tags(content);
    let word_count = content.split_whitespace().count();
    let tasks = extract_tasks(content, start_line);

    Section {
        heading,
        heading_path,
        content: content.to_string(),
        links,
        tags,
        word_count,
        tasks,
    }
}

fn extract_tasks(content: &str, start_line: usize) -> Vec<Task> {
    let mut tasks = Vec::new();
    for (line_offset, line) in content.lines().enumerate() {
        if let Some(caps) = TASK_RE.captures(line) {
            let marker = caps[1].chars().next().unwrap();
            let status = match marker {
                ' ' => TaskStatus::Open,
                'x' | 'X' => TaskStatus::Done,
                '-' => TaskStatus::Canceled,
                c => TaskStatus::Custom(c),
            };
            tasks.push(Task {
                line: start_line + line_offset,
                status,
                text: caps[2].to_string(),
            });
        }
    }
    tasks
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
        let sections = parse_sections(body);

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

    #[test]
    fn task_status_cancelled() {
        // Test - [-] extracts as TaskStatus::Canceled
        let body = "Some text.\n\n- [-] Cancelled task\n- [ ] Open task\n";
        let sections = parse_sections(body);

        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].tasks.len(), 2);

        assert_eq!(sections[0].tasks[0].status, TaskStatus::Canceled);
        assert_eq!(sections[0].tasks[0].text, "Cancelled task");

        assert_eq!(sections[0].tasks[1].status, TaskStatus::Open);
    }

    #[test]
    fn task_status_custom() {
        // Test - [>], - [?], etc. extract as TaskStatus::Custom
        let body = "\
- [>] Deferred task
- [?] Question task
- [!] Important task
- [x] Completed task
";
        let sections = parse_sections(body);

        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].tasks.len(), 4);

        assert_eq!(sections[0].tasks[0].status, TaskStatus::Custom('>'));
        assert_eq!(sections[0].tasks[0].text, "Deferred task");

        assert_eq!(sections[0].tasks[1].status, TaskStatus::Custom('?'));
        assert_eq!(sections[0].tasks[1].text, "Question task");

        assert_eq!(sections[0].tasks[2].status, TaskStatus::Custom('!'));
        assert_eq!(sections[0].tasks[2].text, "Important task");

        assert_eq!(sections[0].tasks[3].status, TaskStatus::Done);
        assert_eq!(sections[0].tasks[3].text, "Completed task");
    }
}
