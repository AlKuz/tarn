//! Markdown rendering for MCP tool responses.

use std::fmt::Write;

use crate::index::NoteResult;
use crate::note_handler::Note;

/// Renders a collection of note results as markdown.
///
/// Pairs `NoteResult` entries with their parsed `Note` content and produces
/// section-level markdown with HTML comment metadata.
///
/// ```markdown
/// <!-- note_path.md | score: 0.92 | tokens: 280 -->
///
/// ## Section Heading
///
/// Section content...
///
/// ---
///
/// <!-- another_note.md | score: 0.78 | tokens: 150 -->
///
/// ## Another Section
///
/// More content...
/// ```
pub struct RenderMarkdown<'a> {
    notes: Vec<RenderNote<'a>>,
}

impl<'a> RenderMarkdown<'a> {
    /// Build a renderer from parallel slices of results and loaded notes.
    ///
    /// Both slices must have the same length (zip semantics).
    pub fn new(results: &'a [NoteResult], notes: &'a [Note]) -> Self {
        let notes = results
            .iter()
            .zip(notes.iter())
            .map(|(result, note)| RenderNote { result, note })
            .collect();
        Self { notes }
    }

    /// Render all notes to a markdown string.
    pub fn render(&self) -> String {
        let mut output = String::new();
        let mut first = true;

        for render_note in &self.notes {
            let rendered = render_note.render_sections();
            for section in rendered {
                if !first {
                    output.push_str("\n---\n\n");
                }
                first = false;
                output.push_str(&section);
            }
        }

        output.trim().to_string()
    }
}

/// A note paired with its parsed content for rendering.
struct RenderNote<'a> {
    /// Search/list result with path, sections, and scores.
    result: &'a NoteResult,
    /// Parsed note content.
    note: &'a Note,
}

impl RenderNote<'_> {
    /// Render each matched section as a standalone markdown block.
    fn render_sections(&self) -> Vec<String> {
        let path_str = self.result.path.to_string();
        let mut sections = Vec::new();

        for section_result in &self.result.sections {
            let resolved = match self.note.sections.iter().find(|s| {
                s.heading_path.len() == section_result.heading_path.len()
                    && s.heading_path
                        .iter()
                        .zip(section_result.heading_path.iter())
                        .all(|(a, b)| a == b)
            }) {
                Some(s) => s,
                None => continue,
            };

            let mut block = String::new();
            let token_count = resolved.word_count();
            // Metadata comment
            if let Some(score) = section_result.score {
                writeln!(
                    block,
                    "<!-- {} | score: {:.2} | tokens: {} -->\n",
                    path_str, score, token_count
                )
                .unwrap();
            } else {
                writeln!(block, "<!-- {} | tokens: {} -->\n", path_str, token_count).unwrap();
            }

            // Section heading (use last element of heading_path, or note filename for root)
            let heading = if section_result.heading_path.is_empty() {
                path_str
                    .strip_suffix(".md")
                    .unwrap_or(&path_str)
                    .split('/')
                    .next_back()
                    .unwrap_or("Root")
                    .to_string()
            } else {
                section_result.heading_path.last().unwrap().clone()
            };

            writeln!(block, "## {}\n", heading).unwrap();
            writeln!(block, "{}", resolved.content.trim()).unwrap();

            sections.push(block);
        }

        sections
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::{RevisionToken, VaultPath};
    use crate::index::{NoteResult, SectionResult};

    fn make_note_result(path: &str, sections: Vec<SectionResult>) -> NoteResult {
        NoteResult {
            path: VaultPath::new(path).unwrap(),
            revision: RevisionToken::from("rev"),
            sections,
        }
    }

    fn make_section(heading_path: Vec<&str>, score: Option<f32>) -> SectionResult {
        SectionResult {
            heading_path: heading_path.into_iter().map(String::from).collect(),
            tags: vec![],
            links: vec![],
            token_count: 10,
            score,
        }
    }

    #[test]
    fn render_root_section_uses_filename() {
        let results = [make_note_result(
            "projects/design.md",
            vec![make_section(vec![], Some(0.5))],
        )];
        let notes = [Note::from("Root content here")];

        let renderer = RenderMarkdown::new(&results, &notes);
        let output = renderer.render();

        assert!(output.contains("## design"));
        assert!(output.contains("Root content here"));
    }

    #[test]
    fn render_without_scores() {
        let results = [make_note_result(
            "note.md",
            vec![make_section(vec!["Heading"], None)],
        )];
        let notes = [Note::from("# Heading\n\nSome content")];

        let renderer = RenderMarkdown::new(&results, &notes);
        let output = renderer.render();

        assert!(output.contains("<!-- note.md | tokens:"));
        assert!(!output.contains("score:"));
    }

    #[test]
    fn render_with_scores() {
        let results = [make_note_result(
            "note.md",
            vec![make_section(vec!["Heading"], Some(0.92))],
        )];
        let notes = [Note::from("# Heading\n\nSome content")];

        let renderer = RenderMarkdown::new(&results, &notes);
        let output = renderer.render();

        assert!(output.contains("score: 0.92"));
    }

    #[test]
    fn render_nested_path_filename() {
        let results = [make_note_result(
            "deep/nested/path/readme.md",
            vec![make_section(vec![], Some(0.5))],
        )];
        let notes = [Note::from("Content of readme")];

        let renderer = RenderMarkdown::new(&results, &notes);
        let output = renderer.render();

        assert!(output.contains("## readme"));
    }
}
