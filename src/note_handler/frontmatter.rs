use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;
use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

static FRONTMATTER_RE: LazyLock<Regex> = LazyLock::new(|| {
    // Handle both Unix (\n) and Windows (\r\n) line endings
    Regex::new(r"(?s)\A---\r?\n(.*?)\r?\n---\r?\n?").expect("valid frontmatter regex")
});

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FrontmatterValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    List(Vec<FrontmatterValue>),
    Map(HashMap<String, FrontmatterValue>),
}

impl fmt::Display for FrontmatterValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let yaml = yaml_serde::to_string(self).map_err(|_| fmt::Error)?;
        write!(f, "{}", yaml.trim_end_matches('\n'))
    }
}

impl FromStr for FrontmatterValue {
    type Err = yaml_serde::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        yaml_serde::from_str(s)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Frontmatter {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,
    #[serde(flatten)]
    pub custom: HashMap<String, FrontmatterValue>,
}

impl fmt::Display for Frontmatter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let yaml = yaml_serde::to_string(self).map_err(|_| fmt::Error)?;
        write!(f, "---\n{yaml}\n---")
    }
}

impl FromStr for Frontmatter {
    type Err = yaml_serde::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        yaml_serde::from_str(s)
    }
}

pub(crate) fn split_frontmatter(content: &str) -> (Option<Frontmatter>, String) {
    let Some(caps) = FRONTMATTER_RE.captures(content) else {
        return (None, content.to_string());
    };

    // Normalize CRLF to LF for YAML parsing
    let yaml_block = caps[1].replace('\r', "");
    let full_match = caps.get(0).unwrap();
    let body = &content[full_match.end()..];

    let frontmatter: Frontmatter = yaml_block.parse().unwrap_or_default();
    (Some(frontmatter), body.to_string())
}

pub(crate) fn try_split_frontmatter(
    content: &str,
) -> Result<(Option<Frontmatter>, String), yaml_serde::Error> {
    let Some(caps) = FRONTMATTER_RE.captures(content) else {
        return Ok((None, content.to_string()));
    };

    // Normalize CRLF to LF for YAML parsing
    let yaml_block = caps[1].replace('\r', "");
    let full_match = caps.get(0).unwrap();
    let body = &content[full_match.end()..];

    let frontmatter: Frontmatter = yaml_block.parse()?;
    Ok((Some(frontmatter), body.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frontmatter_value_display() {
        // Test Display for various FrontmatterValue variants
        assert_eq!(FrontmatterValue::Null.to_string(), "null");
        assert_eq!(FrontmatterValue::Bool(true).to_string(), "true");
        assert_eq!(FrontmatterValue::Bool(false).to_string(), "false");
        assert_eq!(FrontmatterValue::Int(42).to_string(), "42");
        assert_eq!(FrontmatterValue::Float(1.23).to_string(), "1.23");
        assert_eq!(
            FrontmatterValue::Str("hello".to_string()).to_string(),
            "hello"
        );

        let list = FrontmatterValue::List(vec![FrontmatterValue::Int(1), FrontmatterValue::Int(2)]);
        let list_str = list.to_string();
        assert!(list_str.contains("1"));
        assert!(list_str.contains("2"));

        let mut map = HashMap::new();
        map.insert(
            "key".to_string(),
            FrontmatterValue::Str("value".to_string()),
        );
        let map_val = FrontmatterValue::Map(map);
        let map_str = map_val.to_string();
        assert!(map_str.contains("key"));
        assert!(map_str.contains("value"));
    }

    #[test]
    fn frontmatter_value_from_str() {
        // Test FromStr for various YAML values
        let null: FrontmatterValue = "null".parse().unwrap();
        assert_eq!(null, FrontmatterValue::Null);

        let bool_val: FrontmatterValue = "true".parse().unwrap();
        assert_eq!(bool_val, FrontmatterValue::Bool(true));

        let int_val: FrontmatterValue = "42".parse().unwrap();
        assert_eq!(int_val, FrontmatterValue::Int(42));

        let float_val: FrontmatterValue = "1.23".parse().unwrap();
        assert_eq!(float_val, FrontmatterValue::Float(1.23));

        let str_val: FrontmatterValue = "\"hello\"".parse().unwrap();
        assert_eq!(str_val, FrontmatterValue::Str("hello".to_string()));

        let list_val: FrontmatterValue = "[1, 2, 3]".parse().unwrap();
        assert!(matches!(list_val, FrontmatterValue::List(_)));
    }

    #[test]
    fn frontmatter_display() {
        // Test Frontmatter::fmt produces valid YAML block
        let mut frontmatter = Frontmatter::default();
        frontmatter.title = Some("Test Title".to_string());
        frontmatter.tags = vec!["rust".to_string(), "test".to_string()];

        let output = frontmatter.to_string();
        assert!(output.starts_with("---\n"));
        assert!(output.ends_with("\n---"));
        assert!(output.contains("title: Test Title"));
        assert!(output.contains("rust"));
        assert!(output.contains("test"));
    }

    #[test]
    fn try_split_frontmatter_no_frontmatter() {
        // Test content without frontmatter returns Ok(None, content)
        let content = "Just some regular content.\n\nNo frontmatter here.";
        let (frontmatter, body) = try_split_frontmatter(content).unwrap();

        assert!(frontmatter.is_none());
        assert_eq!(body, content);
    }

    #[test]
    fn try_split_frontmatter_valid() {
        // Test valid frontmatter returns Ok(Some(parsed), body)
        let content = "---\ntitle: My Note\ntags:\n  - rust\n  - test\n---\nBody content here.";
        let (frontmatter, body) = try_split_frontmatter(content).unwrap();

        let fm = frontmatter.unwrap();
        assert_eq!(fm.title, Some("My Note".to_string()));
        assert_eq!(fm.tags, vec!["rust", "test"]);
        assert_eq!(body, "Body content here.");
    }

    #[test]
    fn try_split_frontmatter_invalid_yaml() {
        // Test invalid YAML returns error
        let content = "---\n: : invalid yaml [\n---\nBody.";
        let result = try_split_frontmatter(content);
        assert!(result.is_err());
    }
}
