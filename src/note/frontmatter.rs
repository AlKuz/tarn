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
    Number(f64),
    Str(String),
    List(Vec<FrontmatterValue>),
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

pub(crate) fn split_frontmatter(content: &str) -> (Frontmatter, String) {
    let Some(caps) = FRONTMATTER_RE.captures(content) else {
        return (Frontmatter::default(), content.to_string());
    };

    // Normalize CRLF to LF for YAML parsing
    let yaml_block = caps[1].replace('\r', "");
    let full_match = caps.get(0).unwrap();
    let body = &content[full_match.end()..];

    let frontmatter: Frontmatter = yaml_block.parse().unwrap_or_default();
    (frontmatter, body.to_string())
}

pub(crate) fn try_split_frontmatter(
    content: &str,
) -> Result<(Frontmatter, String), yaml_serde::Error> {
    let Some(caps) = FRONTMATTER_RE.captures(content) else {
        return Ok((Frontmatter::default(), content.to_string()));
    };

    // Normalize CRLF to LF for YAML parsing
    let yaml_block = caps[1].replace('\r', "");
    let full_match = caps.get(0).unwrap();
    let body = &content[full_match.end()..];

    let frontmatter: Frontmatter = yaml_block.parse()?;
    Ok((frontmatter, body.to_string()))
}
