use crate::common::*;
use serde_json::{Value, json};

/// Simulates an agent searching for a topic, reading notes via resources, and following links.
#[tokio::test]
async fn search_read_follow_links() {
    let (_tmp, client) = spawn_server(false).await;

    // Step 1: Discover vault structure via resource
    let info = read_resource(&client, "tarn://vault/info").await;
    assert!(info["note_count"].as_u64().unwrap() >= 7);

    // Step 2: Search for "Rust"
    let search = call_tool(&client, "tarn_search_notes", json!({"query": "Rust"})).await;
    assert!(search["total"].as_u64().unwrap() > 0);

    let paths: Vec<&str> = search["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["path"].as_str().unwrap())
        .collect();
    assert!(paths.contains(&"wiki/Rust.md"));

    // Step 3: Read the Rust note via resource
    let note = read_resource(&client, "tarn://note/wiki/Rust.md").await;
    assert_eq!(note["title"], "Rust");
    assert!(
        note["tags"]
            .as_array()
            .unwrap()
            .contains(&Value::String("programming/rust".to_string()))
    );

    // Step 4: Read a linked note (WebApp) via resource
    let webapp = read_resource(&client, "tarn://note/projects/WebApp.md").await;
    assert_eq!(webapp["title"], "WebApp");
    assert!(
        webapp["tags"]
            .as_array()
            .unwrap()
            .contains(&Value::String("project".to_string()))
    );
}

/// Simulates an agent exploring a project folder.
#[tokio::test]
async fn project_exploration_workflow() {
    let (_tmp, client) = spawn_server(false).await;

    // Step 1: Get project vault info
    let info = read_resource(&client, "tarn://vault/info/projects").await;
    assert!(info["note_count"].as_u64().unwrap() >= 1);

    // Step 2: Get tags for the project folder
    let tags = read_resource(&client, "tarn://vault/tags/projects").await;
    assert!(!tags["tags"].as_array().unwrap().is_empty());

    // Step 3: Search all project notes (empty query = list)
    let search = call_tool(
        &client,
        "tarn_search_notes",
        json!({"query": "", "folder": "projects", "limit": 50}),
    )
    .await;
    assert!(search["total"].as_u64().unwrap() >= 1);

    // Step 4: Read each note via resource
    for result in search["results"].as_array().unwrap() {
        let path = result["path"].as_str().unwrap();
        let note = read_resource(&client, &format!("tarn://note/{path}")).await;
        assert!(note["content"].as_str().is_some());
    }
}

/// Simulates an agent creating, reading, and updating notes.
#[tokio::test]
async fn write_workflow() {
    let (_tmp, client) = spawn_server(false).await;

    // Step 1: Create a new note
    let created = call_tool(
        &client,
        "tarn_create_note",
        json!({
            "path": "projects/new-feature.md",
            "content": "---\ntags:\n  - project\n  - active\n---\n# New Feature\n\n## Status\n\nIn progress.\n\n## Tasks\n\n- [ ] Design\n- [ ] Implement"
        }),
    )
    .await;
    let revision = created["revision"].as_str().unwrap().to_string();

    // Step 2: Read it back via resource
    let note = read_resource(&client, "tarn://note/projects/new-feature.md").await;
    assert_eq!(note["title"], "New Feature");
    assert!(
        note["tags"]
            .as_array()
            .unwrap()
            .contains(&Value::String("project".to_string()))
    );

    // Step 3: Update with revision
    let updated = call_tool(
        &client,
        "tarn_update_note",
        json!({
            "path": "projects/new-feature.md",
            "content": "---\ntags:\n  - project\n  - active\n---\n# New Feature\n\n## Status\n\nCompleted.\n\n## Tasks\n\n- [x] Design\n- [x] Implement",
            "revision": revision
        }),
    )
    .await;
    let new_revision = updated["revision"].as_str().unwrap();
    assert_ne!(new_revision, revision);

    // Step 4: Verify via resource
    let final_note = read_resource(&client, "tarn://note/projects/new-feature.md").await;
    assert!(
        final_note["content"]
            .as_str()
            .unwrap()
            .contains("Completed")
    );

    // Step 5: The new note should appear in project search
    let search = call_tool(
        &client,
        "tarn_search_notes",
        json!({"query": "", "folder": "projects", "tag_filter": ["project", "active"]}),
    )
    .await;
    let paths: Vec<&str> = search["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["path"].as_str().unwrap())
        .collect();
    assert!(paths.contains(&"projects/new-feature.md"));
}

/// Full research session combining resources, tools, and tag navigation.
#[tokio::test]
async fn full_research_session() {
    let (_tmp, client) = spawn_server(false).await;

    // Step 1: Discover vault
    let info = read_resource(&client, "tarn://vault/info").await;
    assert!(info["note_count"].as_u64().unwrap() >= 7);

    // Step 2: Search for "API"
    let search = call_tool(&client, "tarn_search_notes", json!({"query": "API"})).await;
    assert!(search["total"].as_u64().unwrap() > 0);

    // Step 3: Read REST API note via resource
    let note = read_resource(&client, "tarn://note/wiki/REST API.md").await;
    assert_eq!(note["title"], "REST API");

    // Step 4: Read HTTP note via resource
    let http = read_resource(&client, "tarn://note/wiki/HTTP.md").await;
    assert_eq!(http["title"], "HTTP");

    // Step 5: Explore programming/web tag
    let tags = call_tool(
        &client,
        "tarn_get_tags",
        json!({"prefix": "programming/web", "include_notes": true}),
    )
    .await;
    let web_tag = tags["tags"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["tag"] == "programming/web")
        .unwrap();
    assert!(web_tag["count"].as_u64().unwrap() >= 2);
}
