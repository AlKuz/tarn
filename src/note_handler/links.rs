use std::fmt;
use std::str::FromStr;
use std::sync::LazyLock;

use regex::Regex;
use thiserror::Error;

// ---------------------------------------------------------------------------
// Static regexes
// ---------------------------------------------------------------------------

// Captures: wiki_embed, wiki_target, wiki_heading, wiki_block, wiki_alias
static WIKILINK_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"(?P<wiki_embed>!?)\[\[",
        r"(?P<wiki_target>[^#|^\]\n]*)",
        r"(?:#(?P<wiki_heading>[^^|#\]\n]*))?",
        r"(?:#?\^(?P<wiki_block>[^|\]\n]*))?",
        r"(?:\|(?P<wiki_alias>[^\]\n]*))?",
        r"\]\]",
    ))
    .expect("valid wikilink regex")
});

// Captures: md_embed, md_text, md_url, md_title
static MD_LINK_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?P<md_embed>!?)\[(?P<md_text>[^]]*)](\((?P<md_url>[^)\s]+)(?:\s+"(?P<md_title>[^"]*)")?\))"#,
    )
    .expect("valid markdown link regex")
});

// Captures: auto_url
static AUTOLINK_URL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"<(?P<auto_url>[a-zA-Z][a-zA-Z0-9+.-]*://[^\s>]+)>")
        .expect("valid autolink URL regex")
});

// Captures: auto_email
static AUTOLINK_EMAIL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"<(?P<auto_email>[^\s@>]+@[^\s>]+)>").expect("valid autolink email regex")
});

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("not a recognized link syntax")]
pub struct ParseLinkError;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WikiLink {
    pub target: String,
    pub alias: Option<String>,
    pub heading: Option<String>,
    pub block_ref: Option<String>,
    pub is_embed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MarkdownLink {
    pub text: String,
    pub url: String,
    pub title: Option<String>,
    pub is_embed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UrlLink {
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EmailLink {
    pub address: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Link {
    /// Obsidian wikilink: `[[target]]`, `[[target|alias]]`, `[[target#heading]]`,
    /// `[[target#^block-id]]`, `[[target#heading#^block-id]]`.
    /// Embeds use `![[target]]`.
    Wiki(WikiLink),
    /// Standard markdown link: `[text](url)`, `[text](url "title")`.
    /// Embeds use `![alt](url)`.
    Markdown(MarkdownLink),
    /// URL autolink: `<https://example.com>`.
    Url(UrlLink),
    /// Email autolink: `<user@example.com>`.
    Email(EmailLink),
}

// ---------------------------------------------------------------------------
// Construction from captures
// ---------------------------------------------------------------------------

fn non_empty(s: &str) -> Option<String> {
    if s.is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}

impl WikiLink {
    fn from_captures(caps: &regex::Captures) -> Self {
        Self {
            is_embed: &caps["wiki_embed"] == "!",
            target: caps["wiki_target"].to_string(),
            heading: caps
                .name("wiki_heading")
                .and_then(|m| non_empty(m.as_str())),
            block_ref: caps.name("wiki_block").and_then(|m| non_empty(m.as_str())),
            alias: caps.name("wiki_alias").and_then(|m| non_empty(m.as_str())),
        }
    }
}

impl MarkdownLink {
    fn from_captures(caps: &regex::Captures) -> Self {
        Self {
            is_embed: &caps["md_embed"] == "!",
            text: caps["md_text"].to_string(),
            url: caps["md_url"].to_string(),
            title: caps.name("md_title").map(|m| m.as_str().to_string()),
        }
    }
}

impl UrlLink {
    fn from_captures(caps: &regex::Captures) -> Self {
        Self {
            url: caps["auto_url"].to_string(),
        }
    }
}

impl EmailLink {
    fn from_captures(caps: &regex::Captures) -> Self {
        Self {
            address: caps["auto_email"].to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// Display
// ---------------------------------------------------------------------------

impl fmt::Display for WikiLink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_embed {
            write!(f, "!")?;
        }
        write!(f, "[[{}", self.target)?;
        if let Some(h) = &self.heading {
            write!(f, "#{h}")?;
        }
        if let Some(br) = &self.block_ref {
            write!(f, "#^{br}")?;
        }
        if let Some(a) = &self.alias {
            write!(f, "|{a}")?;
        }
        write!(f, "]]")
    }
}

impl fmt::Display for MarkdownLink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_embed {
            write!(f, "!")?;
        }
        write!(f, "[{}]({}", self.text, self.url)?;
        if let Some(t) = &self.title {
            write!(f, " \"{t}\"")?;
        }
        write!(f, ")")
    }
}

impl fmt::Display for UrlLink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<{}>", self.url)
    }
}

impl fmt::Display for EmailLink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<{}>", self.address)
    }
}

impl fmt::Display for Link {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Link::Wiki(l) => l.fmt(f),
            Link::Markdown(l) => l.fmt(f),
            Link::Url(l) => l.fmt(f),
            Link::Email(l) => l.fmt(f),
        }
    }
}

// ---------------------------------------------------------------------------
// FromStr
// ---------------------------------------------------------------------------

impl FromStr for Link {
    type Err = ParseLinkError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some(caps) = WIKILINK_RE.captures(s) {
            return Ok(Link::Wiki(WikiLink::from_captures(&caps)));
        }
        if let Some(caps) = MD_LINK_RE.captures(s) {
            return Ok(Link::Markdown(MarkdownLink::from_captures(&caps)));
        }
        if let Some(caps) = AUTOLINK_URL_RE.captures(s) {
            return Ok(Link::Url(UrlLink::from_captures(&caps)));
        }
        if let Some(caps) = AUTOLINK_EMAIL_RE.captures(s) {
            return Ok(Link::Email(EmailLink::from_captures(&caps)));
        }
        Err(ParseLinkError)
    }
}

// ---------------------------------------------------------------------------
// Extraction
// ---------------------------------------------------------------------------

impl Link {
    /// Extract all links from a text block.
    pub fn extract(content: &str) -> Vec<Link> {
        let mut links = Vec::new();

        for caps in WIKILINK_RE.captures_iter(content) {
            links.push(Link::Wiki(WikiLink::from_captures(&caps)));
        }
        for caps in MD_LINK_RE.captures_iter(content) {
            links.push(Link::Markdown(MarkdownLink::from_captures(&caps)));
        }
        for caps in AUTOLINK_URL_RE.captures_iter(content) {
            links.push(Link::Url(UrlLink::from_captures(&caps)));
        }
        for caps in AUTOLINK_EMAIL_RE.captures_iter(content) {
            links.push(Link::Email(EmailLink::from_captures(&caps)));
        }

        links
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_str_wikilink_variants() {
        let link: Link = "[[note]]".parse().unwrap();
        assert!(
            matches!(link, Link::Wiki(ref w) if w.target == "note" && w.alias.is_none() && w.heading.is_none() && w.block_ref.is_none() && !w.is_embed)
        );

        let link: Link = "[[note|alias]]".parse().unwrap();
        assert!(
            matches!(link, Link::Wiki(WikiLink { target, alias, .. }) if target == "note" && alias.as_deref() == Some("alias"))
        );

        let link: Link = "[[note#heading]]".parse().unwrap();
        assert!(
            matches!(link, Link::Wiki(WikiLink { target, heading, .. }) if target == "note" && heading.as_deref() == Some("heading"))
        );

        let link: Link = "[[note^block]]".parse().unwrap();
        assert!(
            matches!(link, Link::Wiki(WikiLink { target, block_ref, .. }) if target == "note" && block_ref.as_deref() == Some("block"))
        );

        let link: Link = "[[note#heading|alias]]".parse().unwrap();
        assert!(
            matches!(link, Link::Wiki(WikiLink { target, heading, alias, .. }) if target == "note" && heading.as_deref() == Some("heading") && alias.as_deref() == Some("alias"))
        );

        let link: Link = "![[embed]]".parse().unwrap();
        assert!(
            matches!(link, Link::Wiki(WikiLink { target, is_embed: true, .. }) if target == "embed")
        );
    }

    #[test]
    fn from_str_wikilink_heading_and_block_ref() {
        let link: Link = "[[note#heading#^block]]".parse().unwrap();
        assert!(matches!(
            link,
            Link::Wiki(ref w)
                if w.target == "note"
                    && w.heading.as_deref() == Some("heading")
                    && w.block_ref.as_deref() == Some("block")
                    && w.alias.is_none()
                    && !w.is_embed
        ));

        // With alias
        let link: Link = "[[note#heading#^block|alias]]".parse().unwrap();
        assert!(matches!(
            link,
            Link::Wiki(WikiLink { target, heading, block_ref, alias, is_embed: false })
                if target == "note"
                    && heading.as_deref() == Some("heading")
                    && block_ref.as_deref() == Some("block")
                    && alias.as_deref() == Some("alias")
        ));

        // Block ref without heading: [[note#^block]]
        let link: Link = "[[note#^block]]".parse().unwrap();
        assert!(matches!(
            link,
            Link::Wiki(ref w)
                if w.target == "note"
                    && w.heading.is_none()
                    && w.block_ref.as_deref() == Some("block")
                    && w.alias.is_none()
                    && !w.is_embed
        ));
    }

    #[test]
    fn from_str_markdown() {
        let link: Link = "[text](./path.md)".parse().unwrap();
        assert!(
            matches!(link, Link::Markdown(ref m) if m.text == "text" && m.url == "./path.md" && m.title.is_none() && !m.is_embed)
        );

        let link: Link = r#"[text](./path.md "a title")"#.parse().unwrap();
        assert!(
            matches!(link, Link::Markdown(MarkdownLink { title, .. }) if title.as_deref() == Some("a title"))
        );

        let link: Link = "![alt](image.png)".parse().unwrap();
        assert!(
            matches!(link, Link::Markdown(MarkdownLink { text, url, is_embed: true, .. }) if text == "alt" && url == "image.png")
        );
    }

    #[test]
    fn from_str_autolink() {
        let link: Link = "<https://example.com>".parse().unwrap();
        assert!(matches!(link, Link::Url(UrlLink { url }) if url == "https://example.com"));

        let link: Link = "<user@example.com>".parse().unwrap();
        assert!(
            matches!(link, Link::Email(EmailLink { address }) if address == "user@example.com")
        );
    }

    #[test]
    fn display_roundtrip() {
        let wiki = Link::Wiki(WikiLink {
            target: "note".into(),
            alias: Some("alias".into()),
            heading: Some("h1".into()),
            block_ref: None,
            is_embed: false,
        });
        assert_eq!(wiki.to_string(), "[[note#h1|alias]]");
        assert_eq!(wiki.to_string().parse::<Link>().unwrap(), wiki);

        let embed_wiki = Link::Wiki(WikiLink {
            target: "note".into(),
            alias: None,
            heading: None,
            block_ref: None,
            is_embed: true,
        });
        assert_eq!(embed_wiki.to_string(), "![[note]]");
        assert_eq!(embed_wiki.to_string().parse::<Link>().unwrap(), embed_wiki);

        let md = Link::Markdown(MarkdownLink {
            text: "click".into(),
            url: "./page.md".into(),
            title: None,
            is_embed: false,
        });
        assert_eq!(md.to_string(), "[click](./page.md)");
        assert_eq!(md.to_string().parse::<Link>().unwrap(), md);

        let embed_md = Link::Markdown(MarkdownLink {
            text: "alt".into(),
            url: "img.png".into(),
            title: None,
            is_embed: true,
        });
        assert_eq!(embed_md.to_string(), "![alt](img.png)");
        assert_eq!(embed_md.to_string().parse::<Link>().unwrap(), embed_md);

        let url = Link::Url(UrlLink {
            url: "https://example.com".into(),
        });
        assert_eq!(url.to_string(), "<https://example.com>");
        assert_eq!(url.to_string().parse::<Link>().unwrap(), url);

        let email = Link::Email(EmailLink {
            address: "user@example.com".into(),
        });
        assert_eq!(email.to_string(), "<user@example.com>");
        assert_eq!(email.to_string().parse::<Link>().unwrap(), email);
    }

    #[test]
    fn display_roundtrip_heading_and_block_ref() {
        let link = Link::Wiki(WikiLink {
            target: "note".into(),
            heading: Some("heading".into()),
            block_ref: Some("block".into()),
            alias: None,
            is_embed: false,
        });
        assert_eq!(link.to_string(), "[[note#heading#^block]]");
        assert_eq!(link.to_string().parse::<Link>().unwrap(), link);

        let link = Link::Wiki(WikiLink {
            target: "note".into(),
            heading: None,
            block_ref: Some("block".into()),
            alias: None,
            is_embed: false,
        });
        assert_eq!(link.to_string(), "[[note#^block]]");
        assert_eq!(link.to_string().parse::<Link>().unwrap(), link);
    }

    #[test]
    fn extract_from_text() {
        let content = "See [[wiki]], [note](./other.md) and [google](https://google.com).\n";
        let links = Link::extract(content);

        assert_eq!(links.len(), 3);
        assert!(matches!(&links[0], Link::Wiki(WikiLink { target, .. }) if target == "wiki"));
        assert!(
            matches!(&links[1], Link::Markdown(MarkdownLink { url, .. }) if url == "./other.md")
        );
        assert!(
            matches!(&links[2], Link::Markdown(MarkdownLink { url, .. }) if url == "https://google.com")
        );
    }

    #[test]
    fn extract_wikilink_variants() {
        let content = "See [[note]], [[note|alias]], [[note#heading]], [[note^block]], and [[note#heading|alias]].\n";
        let links = Link::extract(content);

        assert_eq!(links.len(), 5);
        assert!(matches!(&links[0], Link::Wiki(w) if w.target == "note" && w.alias.is_none()));
        assert!(
            matches!(&links[1], Link::Wiki(WikiLink { target, alias, .. }) if target == "note" && alias.as_deref() == Some("alias"))
        );
        assert!(
            matches!(&links[2], Link::Wiki(WikiLink { target, heading, .. }) if target == "note" && heading.as_deref() == Some("heading"))
        );
        assert!(
            matches!(&links[3], Link::Wiki(WikiLink { target, block_ref, .. }) if target == "note" && block_ref.as_deref() == Some("block"))
        );
        assert!(
            matches!(&links[4], Link::Wiki(WikiLink { target, heading, alias, .. }) if target == "note" && heading.as_deref() == Some("heading") && alias.as_deref() == Some("alias"))
        );
    }
}
