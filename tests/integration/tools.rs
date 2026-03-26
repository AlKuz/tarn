use crate::common::*;
use serde_json::{Value, json};

// =============================================================================
// tarn_read_note
// =============================================================================

#[tokio::test]
async fn read_note_full() {
    let (_tmp, client) = spawn_server(false).await;

    let note = call_tool(
        &client,
        "tarn_read_note",
        json!({"path": "wiki/Rust.md", "include_frontmatter": true}),
    )
    .await;

    assert_eq!(note["title"], "Rust");
    assert!(!note["revision"].as_str().unwrap().is_empty());
    assert!(!note["content"].as_str().unwrap().is_empty());
    assert!(
        note["frontmatter"]["tags"]
            .as_array()
            .unwrap()
            .contains(&Value::String("programming/rust".to_string()))
    );
}

#[tokio::test]
async fn read_note_with_links() {
    let (_tmp, client) = spawn_server(false).await;

    let note = call_tool(
        &client,
        "tarn_read_note",
        json!({"path": "wiki/Rust.md", "include_links": true}),
    )
    .await;

    let links = note["links"].as_array().unwrap();
    let wiki_links: Vec<_> = links.iter().filter(|l| l["type"] == "wiki").collect();
    assert!(!wiki_links.is_empty());
}

#[tokio::test]
async fn read_note_specific_sections() {
    let (_tmp, client) = spawn_server(false).await;

    let note = call_tool(
        &client,
        "tarn_read_note",
        json!({"path": "wiki/Rust.md", "sections": ["Ownership"]}),
    )
    .await;

    let content = note["content"].as_str().unwrap();
    assert!(content.contains("Ownership"));
    assert!(!content.contains("# Lifetimes"));
}

#[tokio::test]
async fn read_note_summary_mode() {
    let (_tmp, client) = spawn_server(false).await;

    let note = call_tool(
        &client,
        "tarn_read_note",
        json!({"path": "wiki/Rust.md", "summary": true}),
    )
    .await;

    assert!(note["content"].is_null() || note.get("content").is_none());
    let sections = note["sections"].as_array().unwrap();
    let headings: Vec<&str> = sections
        .iter()
        .map(|s| s["heading"].as_str().unwrap())
        .collect();
    assert!(headings.contains(&"Rust"));
    assert!(headings.contains(&"Ownership"));
    assert!(headings.contains(&"Lifetimes"));
}

#[tokio::test]
async fn read_note_nested_project() {
    let (_tmp, client) = spawn_server(false).await;

    let note = call_tool(
        &client,
        "tarn_read_note",
        json!({"path": "projects/webapp/development/API Design.md"}),
    )
    .await;

    assert_eq!(note["title"], "API Design");
}

// =============================================================================
// tarn_search_notes
// =============================================================================

#[tokio::test]
async fn search_notes_basic() {
    let (_tmp, client) = spawn_server(false).await;

    let result = call_tool(&client, "tarn_search_notes", json!({"query": "Rust"})).await;

    assert!(result["total"].as_u64().unwrap() > 0);
    let paths: Vec<&str> = result["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["path"].as_str().unwrap())
        .collect();
    assert!(paths.contains(&"wiki/Rust.md"));
}

#[tokio::test]
async fn search_case_insensitive() {
    let (_tmp, client) = spawn_server(false).await;

    let lower = call_tool(
        &client,
        "tarn_search_notes",
        json!({"query": "rust", "limit": 100}),
    )
    .await;
    let upper = call_tool(
        &client,
        "tarn_search_notes",
        json!({"query": "RUST", "limit": 100}),
    )
    .await;

    assert_eq!(lower["total"], upper["total"]);
}

#[tokio::test]
async fn search_within_folder() {
    let (_tmp, client) = spawn_server(false).await;

    let result = call_tool(
        &client,
        "tarn_search_notes",
        json!({"query": "Rust", "folder": "wiki"}),
    )
    .await;

    for r in result["results"].as_array().unwrap() {
        assert!(r["path"].as_str().unwrap().starts_with("wiki/"));
    }
}

#[tokio::test]
async fn search_by_tag_filter() {
    let (_tmp, client) = spawn_server(false).await;

    let result = call_tool(
        &client,
        "tarn_search_notes",
        json!({"query": "", "tag_filter": ["programming/web"], "limit": 50}),
    )
    .await;

    assert!(result["total"].as_u64().unwrap() >= 2);
    for r in result["results"].as_array().unwrap() {
        assert!(
            r["tags"]
                .as_array()
                .unwrap()
                .contains(&Value::String("programming/web".to_string()))
        );
    }
}

#[tokio::test]
async fn search_pagination() {
    let (_tmp, client) = spawn_server(false).await;

    let all = call_tool(
        &client,
        "tarn_search_notes",
        json!({"query": "", "limit": 100}),
    )
    .await;
    let page1 = call_tool(
        &client,
        "tarn_search_notes",
        json!({"query": "", "limit": 2, "offset": 0}),
    )
    .await;
    let page2 = call_tool(
        &client,
        "tarn_search_notes",
        json!({"query": "", "limit": 2, "offset": 2}),
    )
    .await;

    assert_eq!(page1["total"], all["total"]);
    assert_eq!(page2["total"], all["total"]);
    assert_eq!(page1["results"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn search_returns_snippets() {
    let (_tmp, client) = spawn_server(false).await;

    let result = call_tool(&client, "tarn_search_notes", json!({"query": "ownership"})).await;

    assert!(!result["results"].as_array().unwrap().is_empty());
    for r in result["results"].as_array().unwrap() {
        assert!(!r["snippet"].as_str().unwrap().is_empty());
    }
}

#[tokio::test]
async fn search_in_nested_folder() {
    let (_tmp, client) = spawn_server(false).await;

    let result = call_tool(
        &client,
        "tarn_search_notes",
        json!({"query": "API", "folder": "projects/webapp"}),
    )
    .await;

    assert!(result["total"].as_u64().unwrap() >= 1);
    for r in result["results"].as_array().unwrap() {
        assert!(r["path"].as_str().unwrap().starts_with("projects/webapp/"));
    }
}

// =============================================================================
// tarn_list_notes
// =============================================================================

#[tokio::test]
async fn list_notes_recursive() {
    let (_tmp, client) = spawn_server(false).await;

    let list = call_tool(
        &client,
        "tarn_list_notes",
        json!({"recursive": true, "limit": 100}),
    )
    .await;

    assert!(list["total"].as_u64().unwrap() >= 7);
}

#[tokio::test]
async fn list_notes_non_recursive() {
    let (_tmp, client) = spawn_server(false).await;

    let list = call_tool(
        &client,
        "tarn_list_notes",
        json!({"recursive": false, "limit": 100}),
    )
    .await;

    for note in list["notes"].as_array().unwrap() {
        assert!(
            !note["path"].as_str().unwrap().contains('/'),
            "path {} should be in root",
            note["path"]
        );
    }
}

#[tokio::test]
async fn list_notes_in_folder() {
    let (_tmp, client) = spawn_server(false).await;

    let list = call_tool(
        &client,
        "tarn_list_notes",
        json!({"folder": "daily", "recursive": true}),
    )
    .await;

    assert_eq!(list["total"].as_u64().unwrap(), 2);
}

#[tokio::test]
async fn list_notes_sorted_by_title() {
    let (_tmp, client) = spawn_server(false).await;

    let list = call_tool(
        &client,
        "tarn_list_notes",
        json!({"recursive": true, "sort": "title", "limit": 100}),
    )
    .await;

    let titles: Vec<&str> = list["notes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["title"].as_str().unwrap_or(""))
        .collect();
    let mut sorted = titles.clone();
    sorted.sort();
    assert_eq!(titles, sorted);
}

#[tokio::test]
async fn list_notes_tag_filter() {
    let (_tmp, client) = spawn_server(false).await;

    let list = call_tool(
        &client,
        "tarn_list_notes",
        json!({"recursive": true, "tag_filter": ["project", "active"], "limit": 50}),
    )
    .await;

    for note in list["notes"].as_array().unwrap() {
        let tags = note["tags"].as_array().unwrap();
        assert!(tags.contains(&Value::String("project".to_string())));
        assert!(tags.contains(&Value::String("active".to_string())));
    }
}

#[tokio::test]
async fn list_notes_includes_word_count() {
    let (_tmp, client) = spawn_server(false).await;

    let list = call_tool(
        &client,
        "tarn_list_notes",
        json!({"folder": "wiki", "recursive": true}),
    )
    .await;

    for note in list["notes"].as_array().unwrap() {
        assert!(note["word_count"].as_u64().unwrap() > 0);
    }
}

#[tokio::test]
async fn list_notes_nested_folder_recursive() {
    let (_tmp, client) = spawn_server(false).await;

    let list = call_tool(
        &client,
        "tarn_list_notes",
        json!({"folder": "projects/webapp", "recursive": true, "limit": 100}),
    )
    .await;

    assert_eq!(list["total"].as_u64().unwrap(), 5);
    let paths: Vec<&str> = list["notes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["path"].as_str().unwrap())
        .collect();
    assert!(paths.iter().any(|p| p.contains("design")));
    assert!(paths.iter().any(|p| p.contains("development")));
    assert!(paths.iter().any(|p| p.contains("docs")));
}

#[tokio::test]
async fn list_notes_nested_folder_non_recursive() {
    let (_tmp, client) = spawn_server(false).await;

    let list = call_tool(
        &client,
        "tarn_list_notes",
        json!({"folder": "projects/webapp/design", "recursive": false, "limit": 100}),
    )
    .await;

    assert_eq!(list["total"].as_u64().unwrap(), 2);
    for note in list["notes"].as_array().unwrap() {
        let path = note["path"].as_str().unwrap();
        assert!(path.starts_with("projects/webapp/design/"));
        let remaining = path.strip_prefix("projects/webapp/design/").unwrap();
        assert!(!remaining.contains('/'));
    }
}

// =============================================================================
// tarn_get_tags
// =============================================================================

#[tokio::test]
async fn get_tags_all() {
    let (_tmp, client) = spawn_server(false).await;

    let result = call_tool(&client, "tarn_get_tags", json!({})).await;

    let tag_names: Vec<&str> = result["tags"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["tag"].as_str().unwrap())
        .collect();
    assert!(tag_names.contains(&"daily"));
    assert!(tag_names.contains(&"project"));
}

#[tokio::test]
async fn get_tags_with_prefix() {
    let (_tmp, client) = spawn_server(false).await;

    let result = call_tool(&client, "tarn_get_tags", json!({"prefix": "programming"})).await;

    let tag_names: Vec<&str> = result["tags"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["tag"].as_str().unwrap())
        .collect();
    assert!(tag_names.contains(&"programming/rust"));
    assert!(tag_names.contains(&"programming/web"));
}

#[tokio::test]
async fn get_tags_with_notes() {
    let (_tmp, client) = spawn_server(false).await;

    let result = call_tool(
        &client,
        "tarn_get_tags",
        json!({"prefix": "daily", "include_notes": true}),
    )
    .await;

    let daily_tag = result["tags"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["tag"] == "daily")
        .unwrap();
    assert_eq!(daily_tag["count"].as_u64().unwrap(), 3);
    let notes = daily_tag["notes"].as_array().unwrap();
    assert!(
        notes
            .iter()
            .any(|p| p.as_str().unwrap().contains("2024-01-15"))
    );
}

#[tokio::test]
async fn get_tags_hierarchy() {
    let (_tmp, client) = spawn_server(false).await;

    let result = call_tool(&client, "tarn_get_tags", json!({})).await;

    let programming = result["tags"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["tag"] == "programming");
    if let Some(tag) = programming {
        assert!(!tag["children"].as_array().unwrap().is_empty());
    }
}

// =============================================================================
// tarn_create_note
// =============================================================================

#[tokio::test]
async fn create_note_success() {
    let (_tmp, client) = spawn_server(false).await;

    let result = call_tool(
        &client,
        "tarn_create_note",
        json!({"path": "test/new-note.md", "content": "# Test\n\nHello world"}),
    )
    .await;

    assert_eq!(result["path"], "test/new-note.md");
    assert!(!result["revision"].as_str().unwrap().is_empty());

    // Read it back
    let note = call_tool(
        &client,
        "tarn_read_note",
        json!({"path": "test/new-note.md"}),
    )
    .await;
    assert_eq!(note["title"], "Test");
    assert!(note["content"].as_str().unwrap().contains("Hello world"));
}

#[tokio::test]
async fn create_note_existing_fails() {
    let (_tmp, client) = spawn_server(false).await;

    let error = call_tool_expect_error(
        &client,
        "tarn_create_note",
        json!({"path": "wiki/Rust.md", "content": "# Duplicate"}),
    )
    .await;

    assert!(!error.is_empty());
}

// =============================================================================
// tarn_update_note
// =============================================================================

#[tokio::test]
async fn update_note_with_valid_revision() {
    let (_tmp, client) = spawn_server(false).await;

    // Read to get revision
    let note = call_tool(&client, "tarn_read_note", json!({"path": "wiki/Rust.md"})).await;
    let revision = note["revision"].as_str().unwrap();

    // Update
    let result = call_tool(
        &client,
        "tarn_update_note",
        json!({
            "path": "wiki/Rust.md",
            "content": "# Rust\n\nUpdated content.",
            "revision": revision
        }),
    )
    .await;

    assert_eq!(result["path"], "wiki/Rust.md");
    // Revision should change
    assert_ne!(result["revision"].as_str().unwrap(), revision);
}

#[tokio::test]
async fn update_note_wrong_revision_fails() {
    let (_tmp, client) = spawn_server(false).await;

    let error = call_tool_expect_error(
        &client,
        "tarn_update_note",
        json!({
            "path": "wiki/Rust.md",
            "content": "# Bad Update",
            "revision": "wrong-revision-token"
        }),
    )
    .await;

    assert!(!error.is_empty());
}

// =============================================================================
// tarn_replace_in_note
// =============================================================================

#[tokio::test]
async fn replace_in_note_first_mode() {
    let (_tmp, client) = spawn_server(false).await;

    let note = call_tool(&client, "tarn_read_note", json!({"path": "wiki/Rust.md"})).await;
    let revision = note["revision"].as_str().unwrap();

    let result = call_tool(
        &client,
        "tarn_replace_in_note",
        json!({
            "path": "wiki/Rust.md",
            "old": "Rust",
            "new": "Rust (edited)",
            "mode": "first",
            "revision": revision
        }),
    )
    .await;

    assert_eq!(result["path"], "wiki/Rust.md");

    // Verify the replacement
    let updated = call_tool(&client, "tarn_read_note", json!({"path": "wiki/Rust.md"})).await;
    let content = updated["content"].as_str().unwrap();
    assert!(content.contains("Rust (edited)"));
}

#[tokio::test]
async fn replace_in_note_all_mode() {
    let (_tmp, client) = spawn_server(false).await;

    // Create a note with repeated text
    call_tool(
        &client,
        "tarn_create_note",
        json!({"path": "test/replace-all.md", "content": "foo bar foo baz foo"}),
    )
    .await;

    let note = call_tool(
        &client,
        "tarn_read_note",
        json!({"path": "test/replace-all.md"}),
    )
    .await;
    let revision = note["revision"].as_str().unwrap();

    call_tool(
        &client,
        "tarn_replace_in_note",
        json!({
            "path": "test/replace-all.md",
            "old": "foo",
            "new": "qux",
            "mode": "all",
            "revision": revision
        }),
    )
    .await;

    let updated = call_tool(
        &client,
        "tarn_read_note",
        json!({"path": "test/replace-all.md"}),
    )
    .await;
    let content = updated["content"].as_str().unwrap();
    assert!(!content.contains("foo"));
    assert!(content.contains("qux"));
}

#[tokio::test]
async fn replace_in_note_regex_mode() {
    let (_tmp, client) = spawn_server(false).await;

    call_tool(
        &client,
        "tarn_create_note",
        json!({"path": "test/replace-regex.md", "content": "date: 2024-01-15\ndate: 2024-01-16"}),
    )
    .await;

    let note = call_tool(
        &client,
        "tarn_read_note",
        json!({"path": "test/replace-regex.md"}),
    )
    .await;
    let revision = note["revision"].as_str().unwrap();

    call_tool(
        &client,
        "tarn_replace_in_note",
        json!({
            "path": "test/replace-regex.md",
            "old": r"(\d{4})-(\d{2})-(\d{2})",
            "new": "$1/$2/$3",
            "mode": "regex",
            "revision": revision
        }),
    )
    .await;

    let updated = call_tool(
        &client,
        "tarn_read_note",
        json!({"path": "test/replace-regex.md"}),
    )
    .await;
    let content = updated["content"].as_str().unwrap();
    assert!(content.contains("2024/01/15"));
    assert!(content.contains("2024/01/16"));
}

// =============================================================================
// Tool Listing
// =============================================================================

#[tokio::test]
async fn lists_all_tools() {
    let (_tmp, client) = spawn_server(false).await;

    let tools = client.list_all_tools().await.unwrap();
    let names: Vec<&str> = tools.iter().map(|t| t.name.as_ref()).collect();

    assert!(names.contains(&"tarn_read_note"));
    assert!(names.contains(&"tarn_search_notes"));
    assert!(names.contains(&"tarn_list_notes"));
    assert!(names.contains(&"tarn_get_tags"));
    assert!(names.contains(&"tarn_create_note"));
    assert!(names.contains(&"tarn_update_note"));
    assert!(names.contains(&"tarn_replace_in_note"));
    assert_eq!(names.len(), 7);
}

// =============================================================================
// With Index
// =============================================================================

#[tokio::test]
async fn search_with_index() {
    let (_tmp, client) = spawn_server(true).await;

    let result = call_tool(
        &client,
        "tarn_search_notes",
        json!({"query": "Rust ownership"}),
    )
    .await;

    assert!(result["total"].as_u64().unwrap() > 0);
    let paths: Vec<&str> = result["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["path"].as_str().unwrap())
        .collect();
    assert!(
        paths.iter().any(|p| p.contains("Rust")),
        "expected Rust note in results, got {paths:?}"
    );
}

#[tokio::test]
async fn list_notes_with_index() {
    let (_tmp, client) = spawn_server(true).await;

    let list = call_tool(
        &client,
        "tarn_list_notes",
        json!({"recursive": true, "limit": 100}),
    )
    .await;

    assert!(list["total"].as_u64().unwrap() >= 7);
}
