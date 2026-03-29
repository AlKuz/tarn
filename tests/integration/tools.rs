use crate::common::*;
use serde_json::{Value, json};

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
        json!({"query": "web", "tag_filter": ["programming/web"], "limit": 50}),
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

    // Use a broad query that matches many notes for pagination testing
    let all = call_tool(
        &client,
        "tarn_search_notes",
        json!({"query": "project", "limit": 100}),
    )
    .await;
    let total = all["total"].as_u64().unwrap();
    assert!(total >= 3, "need at least 3 results for pagination test");

    let page1 = call_tool(
        &client,
        "tarn_search_notes",
        json!({"query": "project", "limit": 2, "offset": 0}),
    )
    .await;
    let page2 = call_tool(
        &client,
        "tarn_search_notes",
        json!({"query": "project", "limit": 2, "offset": 2}),
    )
    .await;

    assert_eq!(page1["total"].as_u64().unwrap(), total);
    assert_eq!(page2["total"].as_u64().unwrap(), total);
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

    // Read it back via resource
    let note = read_resource(&client, "tarn://note/test/new-note.md").await;
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

    // Read to get revision via resource
    let note = read_resource(&client, "tarn://note/wiki/Rust.md").await;
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

    let note = read_resource(&client, "tarn://note/wiki/Rust.md").await;
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

    // Verify the replacement via resource
    let updated = read_resource(&client, "tarn://note/wiki/Rust.md").await;
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

    let note = read_resource(&client, "tarn://note/test/replace-all.md").await;
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

    let updated = read_resource(&client, "tarn://note/test/replace-all.md").await;
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

    let note = read_resource(&client, "tarn://note/test/replace-regex.md").await;
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

    let updated = read_resource(&client, "tarn://note/test/replace-regex.md").await;
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

    assert!(names.contains(&"tarn_search_notes"));
    assert!(names.contains(&"tarn_get_tags"));
    assert!(names.contains(&"tarn_create_note"));
    assert!(names.contains(&"tarn_update_note"));
    assert!(names.contains(&"tarn_replace_in_note"));
    assert_eq!(names.len(), 5);
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
