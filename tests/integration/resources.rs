use crate::common::*;
use serde_json::Value;

// =============================================================================
// Vault Info
// =============================================================================

#[tokio::test]
async fn vault_info_returns_metadata() {
    let (_tmp, client) = spawn_server(false).await;

    let info = read_resource(&client, "tarn://vault/info").await;

    assert_eq!(info["name"], "vault");
    assert!(info["note_count"].as_u64().unwrap() >= 7);
    assert!(info["tag_count"].as_u64().unwrap() > 0);
    assert_eq!(info["storage_type"], "local");
}

#[tokio::test]
async fn vault_info_scoped_to_folder() {
    let (_tmp, client) = spawn_server(false).await;

    let info = read_resource(&client, "tarn://vault/info/wiki").await;

    assert_eq!(info["folder"], "wiki/");
    assert_eq!(info["note_count"].as_u64().unwrap(), 6);
}

#[tokio::test]
async fn vault_info_scoped_to_nested_folder() {
    let (_tmp, client) = spawn_server(false).await;

    let info = read_resource(&client, "tarn://vault/info/projects/webapp").await;

    assert_eq!(info["folder"], "projects/webapp/");
    assert_eq!(info["note_count"].as_u64().unwrap(), 5);
}

#[tokio::test]
async fn vault_info_deeply_nested() {
    let (_tmp, client) = spawn_server(false).await;

    let info = read_resource(&client, "tarn://vault/info/areas/personal/health").await;

    assert_eq!(info["folder"], "areas/personal/health/");
    assert_eq!(info["note_count"].as_u64().unwrap(), 1);
}

// =============================================================================
// Vault Folders
// =============================================================================

#[tokio::test]
async fn vault_folders_lists_all() {
    let (_tmp, client) = spawn_server(false).await;

    let folders = read_resource(&client, "tarn://vault/folders").await;
    let paths: Vec<&str> = folders["folders"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["path"].as_str().unwrap())
        .collect();

    assert!(paths.contains(&"wiki/"));
    assert!(paths.contains(&"projects/"));
    assert!(paths.contains(&"daily/"));
    assert!(paths.contains(&"templates/"));
}

#[tokio::test]
async fn vault_folders_include_note_counts() {
    let (_tmp, client) = spawn_server(false).await;

    let folders = read_resource(&client, "tarn://vault/folders").await;
    let wiki = folders["folders"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["path"] == "wiki/")
        .unwrap();
    assert_eq!(wiki["note_count"].as_u64().unwrap(), 3);
}

#[tokio::test]
async fn vault_folders_nested_structure() {
    let (_tmp, client) = spawn_server(false).await;

    let folders = read_resource(&client, "tarn://vault/folders").await;
    let paths: Vec<&str> = folders["folders"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["path"].as_str().unwrap())
        .collect();

    assert!(paths.iter().any(|p| p.contains("webapp/design")));
    assert!(paths.iter().any(|p| p.contains("webapp/development")));
    assert!(paths.iter().any(|p| p.contains("programming/rust")));
    assert!(paths.iter().any(|p| p.contains("programming/web")));
    assert!(paths.iter().any(|p| p.contains("areas/work")));
    assert!(paths.iter().any(|p| p.contains("personal/health")));
}

#[tokio::test]
async fn vault_folders_scoped_to_projects() {
    let (_tmp, client) = spawn_server(false).await;

    let folders = read_resource(&client, "tarn://vault/folders/projects").await;
    let design = folders["folders"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["path"].as_str().unwrap().contains("design"));
    assert!(design.is_some());
    assert_eq!(design.unwrap()["note_count"].as_u64().unwrap(), 2);
}

// =============================================================================
// Vault Tags
// =============================================================================

#[tokio::test]
async fn vault_tags_returns_hierarchy() {
    let (_tmp, client) = spawn_server(false).await;

    let tags = read_resource(&client, "tarn://vault/tags").await;
    let tag_names: Vec<&str> = tags["tags"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["tag"].as_str().unwrap())
        .collect();

    assert!(tag_names.contains(&"daily"));
    assert!(tag_names.contains(&"project"));
    assert!(tag_names.iter().any(|t| t.starts_with("programming/")));
}

#[tokio::test]
async fn vault_tags_scoped_to_folder() {
    let (_tmp, client) = spawn_server(false).await;

    let tags = read_resource(&client, "tarn://vault/tags/wiki").await;

    assert_eq!(tags["folder"], "wiki/");
    let tag_names: Vec<&str> = tags["tags"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["tag"].as_str().unwrap())
        .collect();
    assert!(tag_names.iter().any(|t| t.starts_with("programming")));
}

#[tokio::test]
async fn vault_tags_scoped_to_nested_folder() {
    let (_tmp, client) = spawn_server(false).await;

    let tags = read_resource(&client, "tarn://vault/tags/projects/webapp").await;

    assert_eq!(tags["folder"], "projects/webapp/");
    let tag_names: Vec<&str> = tags["tags"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["tag"].as_str().unwrap())
        .collect();
    assert!(tag_names.contains(&"project/webapp"));
}

// =============================================================================
// Note Resource
// =============================================================================

#[tokio::test]
async fn note_resource_reads_content() {
    let (_tmp, client) = spawn_server(false).await;

    let note = read_resource(&client, "tarn://note/wiki/Rust.md").await;

    assert_eq!(note["title"], "Rust");
    assert!(note["token_count"].as_u64().unwrap() > 0);
    assert!(!note["content"].as_str().unwrap().is_empty());
    assert!(!note["revision"].as_str().unwrap().is_empty());
}

#[tokio::test]
async fn note_resource_nested() {
    let (_tmp, client) = spawn_server(false).await;

    let note = read_resource(&client, "tarn://note/projects/WebApp.md").await;

    assert_eq!(note["title"], "WebApp");
    assert!(
        note["tags"]
            .as_array()
            .unwrap()
            .contains(&Value::String("project".to_string()))
    );
}

#[tokio::test]
async fn note_resource_deeply_nested() {
    let (_tmp, client) = spawn_server(false).await;

    let note = read_resource(&client, "tarn://note/areas/personal/health/Fitness Plan.md").await;

    assert_eq!(note["title"], "Fitness Plan");
    assert!(
        note["tags"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t == "health")
    );
}

#[tokio::test]
async fn note_resource_missing_returns_error() {
    let (_tmp, client) = spawn_server(false).await;

    let result = client
        .read_resource(rmcp::model::ReadResourceRequestParams::new(
            "tarn://note/nonexistent.md",
        ))
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn unknown_uri_returns_error() {
    let (_tmp, client) = spawn_server(false).await;

    let result = client
        .read_resource(rmcp::model::ReadResourceRequestParams::new(
            "tarn://unknown/path",
        ))
        .await;
    assert!(result.is_err());
}

// =============================================================================
// Section Resource
// =============================================================================

#[tokio::test]
async fn section_resource_reads_content() {
    let (_tmp, client) = spawn_server(false).await;

    let section =
        read_resource(&client, "tarn://note/wiki/Rust.md#Rust/Ownership").await;

    assert_eq!(section["note_path"], "wiki/Rust.md");
    assert_eq!(
        section["heading_path"],
        Value::Array(vec![
            Value::String("Rust".into()),
            Value::String("Ownership".into()),
        ])
    );
    assert!(
        section["content"]
            .as_str()
            .unwrap()
            .contains("Every value has exactly one owner")
    );
    assert!(section["token_count"].as_u64().unwrap() > 0);
    assert!(!section["revision"].as_str().unwrap().is_empty());
}

#[tokio::test]
async fn section_resource_nested_heading() {
    let (_tmp, client) = spawn_server(false).await;

    let section =
        read_resource(&client, "tarn://note/wiki/Rust.md#Rust/Ownership/Borrowing").await;

    assert_eq!(section["note_path"], "wiki/Rust.md");
    assert_eq!(
        section["heading_path"],
        Value::Array(vec![
            Value::String("Rust".into()),
            Value::String("Ownership".into()),
            Value::String("Borrowing".into()),
        ])
    );
    assert!(
        section["content"]
            .as_str()
            .unwrap()
            .contains("References allow borrowing")
    );
}

#[tokio::test]
async fn section_resource_includes_tags() {
    let (_tmp, client) = spawn_server(false).await;

    let section =
        read_resource(&client, "tarn://note/wiki/Rust.md#Rust/Resources").await;

    let tags: Vec<&str> = section["tags"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t.as_str().unwrap())
        .collect();
    // Frontmatter tags should be present
    assert!(tags.contains(&"programming/rust"));
}

#[tokio::test]
async fn section_resource_includes_links() {
    let (_tmp, client) = spawn_server(false).await;

    let section =
        read_resource(&client, "tarn://note/wiki/Rust.md#Rust/Resources").await;

    let links = section["links"].as_array().unwrap();
    assert!(!links.is_empty());
    let link_types: Vec<&str> = links
        .iter()
        .map(|l| l["link_type"].as_str().unwrap())
        .collect();
    assert!(link_types.contains(&"markdown"));
}

#[tokio::test]
async fn section_resource_not_found_lists_available() {
    let (_tmp, client) = spawn_server(false).await;

    let result = client
        .read_resource(rmcp::model::ReadResourceRequestParams::new(
            "tarn://note/wiki/Rust.md#NonExistent",
        ))
        .await;

    assert!(result.is_err());
    let err = format!("{:?}", result.unwrap_err());
    assert!(err.contains("section not found"));
    assert!(err.contains("Ownership"));
}

#[tokio::test]
async fn section_resource_path_field_includes_fragment() {
    let (_tmp, client) = spawn_server(false).await;

    let section =
        read_resource(&client, "tarn://note/wiki/Rust.md#Rust/Lifetimes").await;

    assert_eq!(section["path"], "wiki/Rust.md#Rust/Lifetimes");
    assert_eq!(section["note_path"], "wiki/Rust.md");
}

// =============================================================================
// List Resources & Templates
// =============================================================================

#[tokio::test]
async fn lists_static_resources() {
    let (_tmp, client) = spawn_server(false).await;

    let resources = client.list_all_resources().await.unwrap();
    let uris: Vec<&str> = resources.iter().map(|r| r.uri.as_str()).collect();

    assert!(uris.contains(&"tarn://vault/info"));
    assert!(uris.contains(&"tarn://vault/tags"));
    assert!(uris.contains(&"tarn://vault/folders"));
}

#[tokio::test]
async fn lists_resource_templates() {
    let (_tmp, client) = spawn_server(false).await;

    let templates = client.list_all_resource_templates().await.unwrap();
    let uris: Vec<&str> = templates.iter().map(|t| t.uri_template.as_str()).collect();

    assert!(uris.contains(&"tarn://vault/info/{folder}"));
    assert!(uris.contains(&"tarn://note/{path}"));
    assert!(uris.contains(&"tarn://note/{path}#{section_path}"));
}
