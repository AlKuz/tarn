//! Task extraction from Markdown checkbox syntax.
//!
//! Parses task items like `- [ ] Open`, `- [x] Done`, `- [-] Canceled`.

use std::sync::LazyLock;

use regex::Regex;

use super::ExtractFrom;

static TASK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^[\t ]*- \[(.)]\s*(.*)$").expect("valid task regex"));

/// A task item extracted from a note (checkbox list item).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Task {
    /// 1-based line number where the task appears.
    pub line: usize,
    /// Task completion status.
    pub status: TaskStatus,
    /// Task description text (without the checkbox marker).
    pub text: String,
}

/// Completion status of a task checkbox.
#[derive(Debug, Clone, PartialEq, Eq)]
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

impl ExtractFrom for Task {
    type Output = Vec<Task>;

    /// Extract tasks from text with line numbers starting at 1.
    fn extract_from(text: &str) -> Self::Output {
        Self::extract_from_with_offset(text, 1)
    }
}

impl Task {
    /// Extract tasks from text with a custom starting line number.
    ///
    /// This is useful when extracting tasks from a section that doesn't
    /// start at line 1 of the document.
    pub fn extract_from_with_offset(content: &str, start_line: usize) -> Vec<Task> {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_status_cancelled() {
        let body = "Some text.\n\n- [-] Cancelled task\n- [ ] Open task\n";
        let tasks = Task::extract_from(body);

        assert_eq!(tasks.len(), 2);

        assert_eq!(tasks[0].status, TaskStatus::Canceled);
        assert_eq!(tasks[0].text, "Cancelled task");
        assert_eq!(tasks[0].line, 3);

        assert_eq!(tasks[1].status, TaskStatus::Open);
        assert_eq!(tasks[1].line, 4);
    }

    #[test]
    fn task_status_custom() {
        let body = "\
- [>] Deferred task
- [?] Question task
- [!] Important task
- [x] Completed task
";
        let tasks = Task::extract_from(body);

        assert_eq!(tasks.len(), 4);

        assert_eq!(tasks[0].status, TaskStatus::Custom('>'));
        assert_eq!(tasks[0].text, "Deferred task");

        assert_eq!(tasks[1].status, TaskStatus::Custom('?'));
        assert_eq!(tasks[1].text, "Question task");

        assert_eq!(tasks[2].status, TaskStatus::Custom('!'));
        assert_eq!(tasks[2].text, "Important task");

        assert_eq!(tasks[3].status, TaskStatus::Done);
        assert_eq!(tasks[3].text, "Completed task");
    }

    #[test]
    fn extract_with_offset() {
        let body = "- [ ] First\n- [x] Second\n";
        let tasks = Task::extract_from_with_offset(body, 10);

        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].line, 10);
        assert_eq!(tasks[1].line, 11);
    }

    #[test]
    fn empty_content_no_tasks() {
        let tasks = Task::extract_from("");
        assert!(tasks.is_empty());
    }

    #[test]
    fn uppercase_x_marks_done() {
        let body = "- [X] Done with uppercase\n";
        let tasks = Task::extract_from(body);
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].status, TaskStatus::Done);
        assert_eq!(tasks[0].text, "Done with uppercase");
    }

    #[test]
    fn task_with_empty_text() {
        let body = "- [x] \n";
        let tasks = Task::extract_from(body);
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].status, TaskStatus::Done);
        assert_eq!(tasks[0].text, "");
    }

    #[test]
    fn non_task_list_items_skipped() {
        let body = "- regular item\n- another item\n- [ ] actual task\n";
        let tasks = Task::extract_from(body);
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].text, "actual task");
    }

    #[test]
    fn indented_tasks() {
        let body = "  - [ ] Indented with spaces\n\t- [x] Indented with tab\n";
        let tasks = Task::extract_from(body);

        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].text, "Indented with spaces");
        assert_eq!(tasks[1].text, "Indented with tab");
    }
}
