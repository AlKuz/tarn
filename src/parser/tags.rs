use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;

static INLINE_TAG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:^|\s)#([a-zA-Z0-9/_-]+)").expect("valid inline tag regex"));

static INLINE_CODE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"``[^`]+``|`[^`]+`").expect("valid inline code regex"));

/// Strip fenced code blocks and inline code spans so that tags within them
/// are not extracted.
fn strip_code(content: &str) -> String {
    let mut result = String::with_capacity(content.len());
    let mut in_code_fence = false;

    for line in content.lines() {
        if line.trim_start().starts_with("```") {
            in_code_fence = !in_code_fence;
            result.push('\n');
            continue;
        }
        if in_code_fence {
            result.push('\n');
            continue;
        }
        // Strip inline code spans
        let stripped = INLINE_CODE_RE.replace_all(line, "");
        result.push_str(&stripped);
        result.push('\n');
    }

    result
}

pub(super) fn extract_inline_tags(content: &str) -> HashSet<String> {
    let stripped = strip_code(content);
    let mut tags = HashSet::new();

    for caps in INLINE_TAG_RE.captures_iter(&stripped) {
        let tag = &caps[1];
        // Exclude pure numbers (#123 issue refs)
        if !tag.chars().all(|c| c.is_ascii_digit()) {
            tags.insert(tag.to_string());
        }
    }

    tags
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_not_extracted_from_code_blocks() {
        let content = "\
Real #tag here.

```
#not-a-tag inside fence
```

Also `#not-inline-tag` in code span.
";
        let tags = extract_inline_tags(content);

        assert!(tags.contains("tag"));
        assert!(!tags.contains("not-a-tag"));
        assert!(!tags.contains("not-inline-tag"));
    }
}
