use crate::common::*;
use serde_json::{Value, json};

// =============================================================================
// tarn_search_notes
// =============================================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn search_notes_basic() {
    let server = spawn_server(false).await;

    let result = call_tool(
        &server.client,
        "tarn_search_notes",
        json!({"query": "Rust"}),
    )
    .await;

    let results = result.as_array().unwrap();
    assert!(!results.is_empty());
    let paths: Vec<&str> = results
        .iter()
        .map(|r| r["path"].as_str().unwrap())
        .collect();
    assert!(paths.contains(&"wiki/Rust.md"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn search_case_insensitive() {
    let server = spawn_server(false).await;

    let lower = call_tool(
        &server.client,
        "tarn_search_notes",
        json!({"query": "rust", "limit": 100}),
    )
    .await;
    let upper = call_tool(
        &server.client,
        "tarn_search_notes",
        json!({"query": "RUST", "limit": 100}),
    )
    .await;

    assert_eq!(
        lower.as_array().unwrap().len(),
        upper.as_array().unwrap().len()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn search_within_folder() {
    let server = spawn_server(false).await;

    // Use inline folder: filter syntax
    let result = call_tool(
        &server.client,
        "tarn_search_notes",
        json!({"query": "Rust folder:wiki"}),
    )
    .await;

    for r in result.as_array().unwrap() {
        assert!(r["path"].as_str().unwrap().starts_with("wiki/"));
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn search_by_tag_filter() {
    let server = spawn_server(false).await;

    // Use inline tag: filter syntax
    let result = call_tool(
        &server.client,
        "tarn_search_notes",
        json!({"query": "web tag:programming/web", "limit": 50}),
    )
    .await;

    let results = result.as_array().unwrap();
    assert!(results.len() >= 2);
    for r in results {
        // Tags are on individual sections in NoteResult
        let has_tag = r["sections"].as_array().unwrap().iter().any(|s| {
            s["tags"]
                .as_array()
                .unwrap()
                .contains(&Value::String("programming/web".to_string()))
        });
        assert!(has_tag);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn search_limit() {
    let server = spawn_server(false).await;

    // Use a broad query that matches many notes
    let all = call_tool(
        &server.client,
        "tarn_search_notes",
        json!({"query": "project", "limit": 100}),
    )
    .await;
    let total = all.as_array().unwrap().len();
    assert!(total >= 3, "need at least 3 results for limit test");

    // Limit to 2 results
    let limited = call_tool(
        &server.client,
        "tarn_search_notes",
        json!({"query": "project", "limit": 2}),
    )
    .await;

    assert_eq!(limited.as_array().unwrap().len(), 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn search_in_nested_folder() {
    let server = spawn_server(false).await;

    // Use inline folder: filter syntax
    let result = call_tool(
        &server.client,
        "tarn_search_notes",
        json!({"query": "API folder:projects/webapp"}),
    )
    .await;

    let results = result.as_array().unwrap();
    assert!(!results.is_empty());
    for r in results {
        assert!(r["path"].as_str().unwrap().starts_with("projects/webapp/"));
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn search_rendered_mode() {
    let server = spawn_server(false).await;

    let result = call_tool_text(
        &server.client,
        "tarn_search_notes",
        json!({"query": "Rust", "rendered": true}),
    )
    .await;

    // Should be markdown with HTML comment metadata
    assert!(
        result.contains("<!-- wiki/Rust.md |"),
        "expected HTML comment metadata with note path"
    );
    // Should have section content with headings
    assert!(result.contains("## "), "expected section headers");
    // Should include scores in metadata comment
    assert!(
        result.contains("| score:"),
        "expected scores in rendered output"
    );
    // Should include token counts
    assert!(
        result.contains("| tokens:"),
        "expected token counts in rendered output"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn search_rendered_mode_no_scores_for_filter_only() {
    let server = spawn_server(false).await;

    // Filter-only query (tag only, no text)
    let result = call_tool_text(
        &server.client,
        "tarn_search_notes",
        json!({"query": "tag:daily", "rendered": true}),
    )
    .await;

    // Should have markdown with HTML comment metadata
    assert!(result.contains("<!-- "), "expected HTML comment metadata");
    // Should have token counts but NO scores (filter-only mode)
    assert!(
        result.contains("| tokens:"),
        "expected token counts in metadata"
    );
    assert!(
        !result.contains("| score:"),
        "filter-only mode should not include scores"
    );
}

// =============================================================================
// tarn_get_tags
// =============================================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn get_tags_all() {
    let server = spawn_server(false).await;

    let result = call_tool(&server.client, "tarn_get_tags", json!({})).await;

    let tag_names: Vec<&str> = result["tags"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["tag"].as_str().unwrap())
        .collect();
    assert!(tag_names.contains(&"daily"));
    assert!(tag_names.contains(&"project"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn get_tags_with_prefix() {
    let server = spawn_server(false).await;

    let result = call_tool(
        &server.client,
        "tarn_get_tags",
        json!({"prefix": "programming"}),
    )
    .await;

    let tag_names: Vec<&str> = result["tags"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["tag"].as_str().unwrap())
        .collect();
    assert!(tag_names.contains(&"programming/rust"));
    assert!(tag_names.contains(&"programming/web"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn get_tags_with_prefix_count() {
    let server = spawn_server(false).await;

    let result = call_tool(&server.client, "tarn_get_tags", json!({"prefix": "daily"})).await;

    let daily_tag = result["tags"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["tag"] == "daily")
        .unwrap();
    assert_eq!(daily_tag["count"].as_u64().unwrap(), 3);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn get_tags_hierarchy() {
    let server = spawn_server(false).await;

    let result = call_tool(&server.client, "tarn_get_tags", json!({})).await;

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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn create_note_success() {
    let server = spawn_server(false).await;

    let result = call_tool(
        &server.client,
        "tarn_create_note",
        json!({"path": "test/new-note.md", "content": "# Test\n\nHello world"}),
    )
    .await;

    assert_eq!(result["path"], "test/new-note.md");

    // Read it back via resource
    let note = read_resource(&server.client, "tarn://note/test/new-note.md").await;
    assert_eq!(note["title"], "Test");
    assert!(note["content"].as_str().unwrap().contains("Hello world"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn create_note_rejects_existing() {
    let server = spawn_server(false).await;

    let error = call_tool_expect_error(
        &server.client,
        "tarn_create_note",
        json!({"path": "wiki/Rust.md", "content": "# Overwritten"}),
    )
    .await;

    assert!(
        error.contains("already exists"),
        "expected 'already exists' error, got: {error}"
    );
}

// =============================================================================
// tarn_update_note
// =============================================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn update_note_succeeds() {
    let server = spawn_server(false).await;

    let result = call_tool(
        &server.client,
        "tarn_update_note",
        json!({
            "path": "wiki/Rust.md",
            "content": "# Rust\n\nUpdated content."
        }),
    )
    .await;

    assert_eq!(result["path"], "wiki/Rust.md");

    let updated = read_resource(&server.client, "tarn://note/wiki/Rust.md").await;
    assert!(
        updated["content"]
            .as_str()
            .unwrap()
            .contains("Updated content.")
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn update_untracked_note_creates() {
    let server = spawn_server(false).await;

    let result = call_tool(
        &server.client,
        "tarn_update_note",
        json!({
            "path": "wiki/NewNote.md",
            "content": "# New Note"
        }),
    )
    .await;

    assert_eq!(result["path"], "wiki/NewNote.md");
}

// =============================================================================
// tarn_replace_in_note
// =============================================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn replace_in_note_first_mode() {
    let server = spawn_server(false).await;

    let result = call_tool(
        &server.client,
        "tarn_replace_in_note",
        json!({
            "path": "wiki/Rust.md",
            "old": "Rust",
            "new": "Rust (edited)",
            "mode": "first"
        }),
    )
    .await;

    assert_eq!(result["path"], "wiki/Rust.md");

    // Verify the replacement via resource
    let updated = read_resource(&server.client, "tarn://note/wiki/Rust.md").await;
    let content = updated["content"].as_str().unwrap();
    assert!(content.contains("Rust (edited)"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn replace_in_note_all_mode() {
    let server = spawn_server(false).await;

    // Create a note with repeated text
    call_tool(
        &server.client,
        "tarn_create_note",
        json!({"path": "test/replace-all.md", "content": "foo bar foo baz foo"}),
    )
    .await;

    call_tool(
        &server.client,
        "tarn_replace_in_note",
        json!({
            "path": "test/replace-all.md",
            "old": "foo",
            "new": "qux",
            "mode": "all"
        }),
    )
    .await;

    let updated = read_resource(&server.client, "tarn://note/test/replace-all.md").await;
    let content = updated["content"].as_str().unwrap();
    assert!(!content.contains("foo"));
    assert!(content.contains("qux"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn replace_in_note_regex_mode() {
    let server = spawn_server(false).await;

    call_tool(
        &server.client,
        "tarn_create_note",
        json!({"path": "test/replace-regex.md", "content": "date: 2024-01-15\ndate: 2024-01-16"}),
    )
    .await;

    call_tool(
        &server.client,
        "tarn_replace_in_note",
        json!({
            "path": "test/replace-regex.md",
            "old": r"(\d{4})-(\d{2})-(\d{2})",
            "new": "$1/$2/$3",
            "mode": "regex"
        }),
    )
    .await;

    let updated = read_resource(&server.client, "tarn://note/test/replace-regex.md").await;
    let content = updated["content"].as_str().unwrap();
    assert!(content.contains("2024/01/15"));
    assert!(content.contains("2024/01/16"));
}

// =============================================================================
// tarn_create_note — with frontmatter
// =============================================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn create_note_with_frontmatter() {
    let server = spawn_server(false).await;

    let result = call_tool(
        &server.client,
        "tarn_create_note",
        json!({
            "path": "test/fm-note.md",
            "content": "# FM Note\n\nBody content.",
            "frontmatter": {"title": "FM Note", "tags": ["test", "frontmatter"]}
        }),
    )
    .await;

    assert_eq!(result["path"], "test/fm-note.md");

    let note = read_resource(&server.client, "tarn://note/test/fm-note.md").await;
    assert_eq!(note["title"], "FM Note");
    let content = note["content"].as_str().unwrap();
    assert!(content.contains("# FM Note"));
    let tags = note["tags"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t.as_str().unwrap())
        .collect::<Vec<_>>();
    assert!(tags.contains(&"test"));
    assert!(tags.contains(&"frontmatter"));
}

// =============================================================================
// tarn_update_note — append mode
// =============================================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn update_note_append_mode() {
    let server = spawn_server(false).await;

    call_tool(
        &server.client,
        "tarn_create_note",
        json!({"path": "test/append.md", "content": "# Append\n\nOriginal."}),
    )
    .await;

    call_tool(
        &server.client,
        "tarn_update_note",
        json!({
            "path": "test/append.md",
            "content": "\n## Added\n\nAppended content.",
            "mode": "append"
        }),
    )
    .await;

    let note = read_resource(&server.client, "tarn://note/test/append.md").await;
    let content = note["content"].as_str().unwrap();
    assert!(content.contains("Original."));
    assert!(content.contains("Appended content."));
}

// =============================================================================
// tarn_update_frontmatter
// =============================================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn update_frontmatter_set_and_remove() {
    let server = spawn_server(false).await;

    // Create a note with frontmatter
    call_tool(
        &server.client,
        "tarn_create_note",
        json!({
            "path": "test/fm-update.md",
            "content": "# FM Update",
            "frontmatter": {"title": "Original", "status": "draft"}
        }),
    )
    .await;

    // Update: set description, remove status
    let result = call_tool(
        &server.client,
        "tarn_update_frontmatter",
        json!({
            "path": "test/fm-update.md",
            "set": {"description": "A new description"},
            "remove": ["status"]
        }),
    )
    .await;

    assert_eq!(result["path"], "test/fm-update.md");

    let note = read_resource(&server.client, "tarn://note/test/fm-update.md").await;
    let fm = &note["frontmatter"];
    assert_eq!(fm["title"], "Original"); // preserved
    assert_eq!(fm["description"], "A new description"); // added
    assert!(fm.get("status").is_none() || fm["status"].is_null()); // removed
}

// =============================================================================
// tarn_delete_note
// =============================================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn delete_note_success() {
    let server = spawn_server(false).await;

    call_tool(
        &server.client,
        "tarn_create_note",
        json!({"path": "test/to-delete.md", "content": "# Delete Me"}),
    )
    .await;

    let result = call_tool(
        &server.client,
        "tarn_delete_note",
        json!({"path": "test/to-delete.md"}),
    )
    .await;

    assert_eq!(result["path"], "test/to-delete.md");
    assert_eq!(result["deleted"], true);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn delete_note_nonexistent_fails() {
    let server = spawn_server(false).await;

    let error = call_tool_expect_error(
        &server.client,
        "tarn_delete_note",
        json!({"path": "nonexistent.md"}),
    )
    .await;

    assert!(
        error.contains("not found") || error.contains("NoteNotFound"),
        "expected not found error, got: {error}"
    );
}

// =============================================================================
// tarn_rename_note
// =============================================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rename_note_success() {
    let server = spawn_server(false).await;

    call_tool(
        &server.client,
        "tarn_create_note",
        json!({"path": "test/old-name.md", "content": "# Old Name"}),
    )
    .await;

    let result = call_tool(
        &server.client,
        "tarn_rename_note",
        json!({"path": "test/old-name.md", "new_path": "test/new-name.md"}),
    )
    .await;

    assert_eq!(result["old_path"], "test/old-name.md");
    assert_eq!(result["new_path"], "test/new-name.md");

    // New path should be readable
    let note = read_resource(&server.client, "tarn://note/test/new-name.md").await;
    assert_eq!(note["title"], "Old Name");
}

// =============================================================================
// Tool Listing
// =============================================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn lists_all_tools() {
    let server = spawn_server(false).await;

    let tools = server.client.list_all_tools().await.unwrap();
    let names: Vec<&str> = tools.iter().map(|t| t.name.as_ref()).collect();

    assert!(names.contains(&"tarn_search_notes"));
    assert!(names.contains(&"tarn_get_tags"));
    assert!(names.contains(&"tarn_create_note"));
    assert!(names.contains(&"tarn_update_note"));
    assert!(names.contains(&"tarn_replace_in_note"));
    assert!(names.contains(&"tarn_update_frontmatter"));
    assert!(names.contains(&"tarn_delete_note"));
    assert!(names.contains(&"tarn_rename_note"));
    assert_eq!(names.len(), 8);
}

// =============================================================================
// With Index
// =============================================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn search_with_index() {
    let server = spawn_server(true).await;

    let result = call_tool(
        &server.client,
        "tarn_search_notes",
        json!({"query": "Rust ownership"}),
    )
    .await;

    let results = result.as_array().unwrap();
    assert!(!results.is_empty());
    let paths: Vec<&str> = results
        .iter()
        .map(|r| r["path"].as_str().unwrap())
        .collect();
    assert!(
        paths.iter().any(|p| p.contains("Rust")),
        "expected Rust note in results, got {paths:?}"
    );
}
