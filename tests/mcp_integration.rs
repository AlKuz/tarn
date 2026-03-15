//! MCP integration tests simulating agent workflows against the test vault.
//!
//! These tests exercise the MCP layer: tools, resources, and prompts.
//! - Resources and prompts: tested via MCP API directly
//! - Tools: metadata verified via tool router, functionality tested via TarnCore

use std::path::PathBuf;
use std::sync::Arc;

use rmcp::model::{JsonObject, ResourceContents};
use serde_json::Value;

use tarn::TarnBuilder;
use tarn::common::VaultPath;
use tarn::mcp::TarnMcpServer;

fn folder(path: &str) -> VaultPath {
    VaultPath::new(format!("{}/", path)).unwrap()
}

fn vault_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/vault")
}

fn create_server() -> TarnMcpServer {
    let core = TarnBuilder::local(vault_path()).build();
    TarnMcpServer::new(Arc::new(core))
}

fn create_core() -> tarn::TarnCore {
    TarnBuilder::local(vault_path()).build()
}

/// Extract JSON from resource result
fn parse_resource_result(result: &rmcp::model::ReadResourceResult) -> Value {
    let content = result.contents.first().unwrap();
    match content {
        ResourceContents::TextResourceContents { text, .. } => serde_json::from_str(text).unwrap(),
        _ => panic!("expected text resource content"),
    }
}

// =============================================================================
// MCP Resources: Vault Discovery
// =============================================================================

mod resources_vault_info {
    use super::*;

    #[tokio::test]
    async fn reads_vault_info() {
        let server = create_server();

        let result = server
            .read_resource_by_uri("tarn://vault/info")
            .await
            .unwrap();
        let info = parse_resource_result(&result);

        assert_eq!(info["name"], "vault");
        assert!(info["note_count"].as_u64().unwrap() >= 7);
        assert!(info["tag_count"].as_u64().unwrap() > 0);
        assert_eq!(info["storage_type"], "local");
    }

    #[tokio::test]
    async fn scopes_to_folder() {
        let server = create_server();

        let result = server
            .read_resource_by_uri("tarn://vault/info/wiki")
            .await
            .unwrap();
        let info = parse_resource_result(&result);

        assert_eq!(info["folder"], "wiki/");
        // 3 top-level + 3 nested (programming/rust/2 + programming/web/1)
        assert_eq!(info["note_count"].as_u64().unwrap(), 6);
    }

    #[tokio::test]
    async fn scopes_to_projects_folder() {
        let server = create_server();

        let result = server
            .read_resource_by_uri("tarn://vault/info/projects")
            .await
            .unwrap();
        let info = parse_resource_result(&result);

        assert_eq!(info["folder"], "projects/");
        assert!(info["note_count"].as_u64().unwrap() >= 1);
    }
}

mod resources_vault_folders {
    use super::*;

    #[tokio::test]
    async fn lists_all_folders() {
        let server = create_server();

        let result = server
            .read_resource_by_uri("tarn://vault/folders")
            .await
            .unwrap();
        let folders = parse_resource_result(&result);

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
    async fn includes_note_counts() {
        let server = create_server();

        let result = server
            .read_resource_by_uri("tarn://vault/folders")
            .await
            .unwrap();
        let folders = parse_resource_result(&result);

        let wiki = folders["folders"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["path"] == "wiki/")
            .unwrap();
        assert_eq!(wiki["note_count"].as_u64().unwrap(), 3);

        let daily = folders["folders"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["path"] == "daily/")
            .unwrap();
        assert_eq!(daily["note_count"].as_u64().unwrap(), 2);
    }
}

mod resources_vault_tags {
    use super::*;

    #[tokio::test]
    async fn reads_tag_hierarchy() {
        let server = create_server();

        let result = server
            .read_resource_by_uri("tarn://vault/tags")
            .await
            .unwrap();
        let tags = parse_resource_result(&result);

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
    async fn scopes_tags_to_folder() {
        let server = create_server();

        let result = server
            .read_resource_by_uri("tarn://vault/tags/wiki")
            .await
            .unwrap();
        let tags = parse_resource_result(&result);

        assert_eq!(tags["folder"], "wiki/");
        let tag_names: Vec<&str> = tags["tags"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["tag"].as_str().unwrap())
            .collect();

        // Wiki notes have programming/* tags
        assert!(tag_names.iter().any(|t| t.starts_with("programming")));
    }
}

mod resources_note {
    use super::*;

    #[tokio::test]
    async fn reads_note_as_resource() {
        let server = create_server();

        let result = server
            .read_resource_by_uri("tarn://note/wiki/Rust.md")
            .await
            .unwrap();
        let note = parse_resource_result(&result);

        assert_eq!(note["title"], "Rust");
        assert!(note["word_count"].as_u64().unwrap() > 0);
        assert!(!note["content"].as_str().unwrap().is_empty());
        assert!(!note["revision"].as_str().unwrap().is_empty());
    }

    #[tokio::test]
    async fn reads_nested_note() {
        let server = create_server();

        let result = server
            .read_resource_by_uri("tarn://note/projects/WebApp.md")
            .await
            .unwrap();
        let note = parse_resource_result(&result);

        assert_eq!(note["title"], "WebApp");
        assert!(
            note["frontmatter"]["tags"]
                .as_array()
                .unwrap()
                .contains(&Value::String("project".to_string()))
        );
    }

    #[tokio::test]
    async fn returns_error_for_missing() {
        let server = create_server();

        let result = server
            .read_resource_by_uri("tarn://note/nonexistent.md")
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn returns_error_for_unknown_uri() {
        let server = create_server();

        let result = server.read_resource_by_uri("tarn://unknown/path").await;

        assert!(result.is_err());
    }
}

// =============================================================================
// MCP Prompts
// =============================================================================

mod prompts {
    use super::*;

    #[tokio::test]
    async fn explore_topic_generates_messages() {
        let server = create_server();

        let mut args = JsonObject::default();
        args.insert("topic".into(), Value::String("ownership".into()));

        let result = server
            .get_prompt_by_name("tarn_explore_topic", &args)
            .unwrap();

        assert!(result.description.unwrap().contains("ownership"));
        assert!(!result.messages.is_empty());

        // First message should be a resource link to vault info
        let first = &result.messages[0];
        assert!(matches!(
            first.content,
            rmcp::model::PromptMessageContent::ResourceLink { .. }
        ));
    }

    #[tokio::test]
    async fn explore_topic_with_folder() {
        let server = create_server();

        let mut args = JsonObject::default();
        args.insert("topic".into(), Value::String("Rust".into()));
        args.insert("folder".into(), Value::String("wiki".into()));

        let result = server
            .get_prompt_by_name("tarn_explore_topic", &args)
            .unwrap();

        assert!(result.description.unwrap().contains("Rust"));
    }

    #[tokio::test]
    async fn summarize_project_generates_messages() {
        let server = create_server();

        let mut args = JsonObject::default();
        args.insert("folder".into(), Value::String("projects".into()));

        let result = server
            .get_prompt_by_name("tarn_summarize_project", &args)
            .unwrap();

        assert!(result.description.unwrap().contains("projects"));
        assert!(result.messages.len() >= 2);
    }

    #[tokio::test]
    async fn requires_topic_argument() {
        let server = create_server();

        let args = JsonObject::default();
        let result = server.get_prompt_by_name("tarn_explore_topic", &args);

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn requires_folder_argument_for_project() {
        let server = create_server();

        let args = JsonObject::default();
        let result = server.get_prompt_by_name("tarn_summarize_project", &args);

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn unknown_prompt_returns_error() {
        let server = create_server();

        let args = JsonObject::default();
        let result = server.get_prompt_by_name("unknown_prompt", &args);

        assert!(result.is_err());
    }
}

// =============================================================================
// Agent Workflow: Research Topic via Core
// =============================================================================
// Simulates tools calling TarnCore methods

mod workflow_research_topic {
    use super::*;

    #[tokio::test]
    async fn search_and_read_rust_notes() {
        let core = create_core();

        // Agent searches for "Rust"
        let search = core.search_notes("Rust", None, None, 10, 0).await.unwrap();
        assert!(search.total > 0);

        let paths: Vec<&str> = search.results.iter().map(|r| r.path.as_str()).collect();
        assert!(paths.contains(&"wiki/Rust.md"));

        // Agent reads the main Rust note
        let note = core
            .read_note("wiki/Rust.md", None, true, true, false)
            .await
            .unwrap();

        assert_eq!(note.title, Some("Rust".to_string()));
        assert!(
            note.frontmatter
                .unwrap()
                .tags
                .contains(&"programming/rust".to_string())
        );

        // Agent finds links to follow
        let links = note.links.unwrap();
        let wiki_links: Vec<_> = links.iter().filter(|l| l.link_type == "wiki").collect();
        assert!(!wiki_links.is_empty());
    }

    #[tokio::test]
    async fn follow_links_between_notes() {
        let core = create_core();

        // Read Rust note with links
        let rust = core
            .read_note("wiki/Rust.md", None, false, true, false)
            .await
            .unwrap();

        let links = rust.links.unwrap();
        let webapp_link = links.iter().find(|l| l.target.contains("WebApp"));
        assert!(webapp_link.is_some());

        // Follow link to WebApp
        let webapp = core
            .read_note("projects/WebApp.md", None, true, true, false)
            .await
            .unwrap();

        assert_eq!(webapp.title, Some("WebApp".to_string()));
        assert!(
            webapp
                .frontmatter
                .unwrap()
                .tags
                .contains(&"project".to_string())
        );
    }

    #[tokio::test]
    async fn get_section_summary() {
        let core = create_core();

        let note = core
            .read_note("wiki/Rust.md", None, false, false, true)
            .await
            .unwrap();

        assert!(note.content.is_none());
        let sections = note.sections.unwrap();
        let headings: Vec<&str> = sections.iter().map(|s| s.heading.as_str()).collect();

        assert!(headings.contains(&"Rust"));
        assert!(headings.contains(&"Ownership"));
        assert!(headings.contains(&"Lifetimes"));
    }

    #[tokio::test]
    async fn read_specific_sections() {
        let core = create_core();

        let note = core
            .read_note(
                "wiki/Rust.md",
                Some(&["Ownership".to_string()]),
                false,
                false,
                false,
            )
            .await
            .unwrap();

        let content = note.content.unwrap();
        assert!(content.contains("Ownership"));
        assert!(!content.contains("# Lifetimes"));
    }
}

// =============================================================================
// Agent Workflow: Explore Project
// =============================================================================

mod workflow_explore_project {
    use super::*;

    #[tokio::test]
    async fn list_and_analyze_project_notes() {
        let core = create_core();

        // List project notes
        let list = core
            .list_notes(Some(&folder("projects")), true, None, None, 50, 0)
            .await
            .unwrap();

        assert!(list.total >= 1);
        let paths: Vec<&str> = list.notes.iter().map(|n| n.path.as_str()).collect();
        assert!(paths.contains(&"projects/WebApp.md"));

        // Read project with sections
        let project = core
            .read_note(
                "projects/WebApp.md",
                Some(&["Tasks".to_string(), "Decisions".to_string()]),
                false,
                false,
                false,
            )
            .await
            .unwrap();

        let content = project.content.unwrap();
        assert!(content.contains("Tasks"));
        assert!(content.contains("Decisions"));
    }

    #[tokio::test]
    async fn filter_by_project_tags() {
        let core = create_core();

        let list = core
            .list_notes(
                None,
                true,
                Some(&["project".to_string(), "active".to_string()]),
                None,
                50,
                0,
            )
            .await
            .unwrap();

        for note in &list.notes {
            assert!(note.tags.contains(&"project".to_string()));
            assert!(note.tags.contains(&"active".to_string()));
        }
    }
}

// =============================================================================
// Agent Workflow: Daily Note Review
// =============================================================================

mod workflow_daily_review {
    use super::*;

    #[tokio::test]
    async fn list_daily_notes() {
        let core = create_core();

        let list = core
            .list_notes(Some(&folder("daily")), true, None, None, 50, 0)
            .await
            .unwrap();

        assert_eq!(list.total, 2);
    }

    #[tokio::test]
    async fn get_daily_tag_with_notes() {
        let core = create_core();

        let tags = core.get_tags(Some("daily"), true).await.unwrap();

        let daily_tag = tags.tags.iter().find(|t| t.tag == "daily").unwrap();
        // 3 notes: 2 daily notes + 1 daily template
        assert_eq!(daily_tag.count, 3);

        let note_paths = daily_tag.notes.as_ref().unwrap();
        assert!(note_paths.iter().any(|p| p.contains("2024-01-15")));
        assert!(note_paths.iter().any(|p| p.contains("2024-01-14")));
    }

    #[tokio::test]
    async fn read_daily_note_with_links() {
        let core = create_core();

        let note = core
            .read_note("daily/2024-01-15.md", None, true, true, false)
            .await
            .unwrap();

        assert_eq!(note.title, Some("2024-01-15".to_string()));

        let links = note.links.unwrap();
        let targets: Vec<&str> = links.iter().map(|l| l.target.as_str()).collect();

        assert!(targets.iter().any(|t| t.contains("WebApp")));
        assert!(targets.iter().any(|t| t.contains("Rust")));
    }
}

// =============================================================================
// Agent Workflow: Tag-Based Navigation
// =============================================================================

mod workflow_tag_navigation {
    use super::*;

    #[tokio::test]
    async fn discover_programming_tags() {
        let core = create_core();

        let tags = core.get_tags(Some("programming"), false).await.unwrap();

        let tag_names: Vec<&str> = tags.tags.iter().map(|t| t.tag.as_str()).collect();
        assert!(tag_names.contains(&"programming/rust"));
        assert!(tag_names.contains(&"programming/web"));
    }

    #[tokio::test]
    async fn search_by_tag_filter() {
        let core = create_core();

        let results = core
            .search_notes("", None, Some(&["programming/web".to_string()]), 50, 0)
            .await
            .unwrap();

        assert!(results.total >= 2);
        for note in &results.results {
            assert!(note.tags.contains(&"programming/web".to_string()));
        }
    }

    #[tokio::test]
    async fn tag_parent_child_relationships() {
        let core = create_core();

        let tags = core.get_tags(None, false).await.unwrap();

        // Check that programming has children
        if let Some(programming) = tags.tags.iter().find(|t| t.tag == "programming") {
            // It should have children like programming/rust, programming/web
            assert!(!programming.children.is_empty());
        }
    }
}

// =============================================================================
// Agent Workflow: Full Research Session
// =============================================================================

mod workflow_full_session {
    use super::*;

    #[tokio::test]
    async fn complete_api_research_session() {
        let core = create_core();
        let server = create_server();

        // Step 1: Agent uses resource to discover vault structure
        let vault_info = server
            .read_resource_by_uri("tarn://vault/info")
            .await
            .unwrap();
        let info = parse_resource_result(&vault_info);
        assert!(info["note_count"].as_u64().unwrap() >= 7);

        // Step 2: Agent searches for "API"
        let search = core.search_notes("API", None, None, 10, 0).await.unwrap();
        assert!(search.total > 0);

        // Step 3: Agent reads REST API note
        let note = core
            .read_note("wiki/REST API.md", None, true, true, false)
            .await
            .unwrap();
        assert_eq!(note.title, Some("REST API".to_string()));

        // Step 4: Agent finds HTTP link
        let links = note.links.unwrap();
        let http_link = links.iter().find(|l| l.target.contains("HTTP"));
        assert!(http_link.is_some());

        // Step 5: Agent reads HTTP note summary
        let http = core
            .read_note("wiki/HTTP.md", None, false, false, true)
            .await
            .unwrap();
        assert_eq!(http.title, Some("HTTP".to_string()));
        assert!(http.content.is_none()); // Summary mode

        // Step 6: Agent explores programming/web tag
        let tags = core.get_tags(Some("programming/web"), true).await.unwrap();
        let web_tag = tags
            .tags
            .iter()
            .find(|t| t.tag == "programming/web")
            .unwrap();
        assert!(web_tag.count >= 2);
    }

    #[tokio::test]
    async fn project_analysis_with_prompt() {
        let core = create_core();
        let server = create_server();

        // Step 1: Get prompt for project summarization
        let mut args = JsonObject::default();
        args.insert("folder".into(), Value::String("projects".into()));

        let prompt = server
            .get_prompt_by_name("tarn_summarize_project", &args)
            .unwrap();
        assert!(!prompt.messages.is_empty());

        // Step 2: Following prompt guidance - get vault info for projects
        let info = server
            .read_resource_by_uri("tarn://vault/info/projects")
            .await
            .unwrap();
        let project_info = parse_resource_result(&info);
        assert!(project_info["note_count"].as_u64().unwrap() >= 1);

        // Step 3: Get tags for projects
        let tags = server
            .read_resource_by_uri("tarn://vault/tags/projects")
            .await
            .unwrap();
        let project_tags = parse_resource_result(&tags);
        assert!(!project_tags["tags"].as_array().unwrap().is_empty());

        // Step 4: List all project notes
        let list = core
            .list_notes(Some(&folder("projects")), true, None, None, 50, 0)
            .await
            .unwrap();
        assert!(list.total >= 1);

        // Step 5: Read each project note summary
        for note_entry in &list.notes {
            let note = core
                .read_note(&note_entry.path, None, true, false, true)
                .await
                .unwrap();
            assert!(note.sections.is_some());
        }
    }
}

// =============================================================================
// Search Functionality
// =============================================================================

mod search {
    use super::*;

    #[tokio::test]
    async fn case_insensitive_search() {
        let core = create_core();

        let lower = core.search_notes("rust", None, None, 100, 0).await.unwrap();
        let upper = core.search_notes("RUST", None, None, 100, 0).await.unwrap();

        assert_eq!(lower.total, upper.total);
    }

    #[tokio::test]
    async fn search_within_folder() {
        let core = create_core();

        let results = core
            .search_notes("Rust", Some(&folder("wiki")), None, 100, 0)
            .await
            .unwrap();

        for result in &results.results {
            assert!(result.path.starts_with("wiki/"));
        }
    }

    #[tokio::test]
    async fn search_returns_snippets() {
        let core = create_core();

        let results = core
            .search_notes("ownership", None, None, 100, 0)
            .await
            .unwrap();

        assert!(!results.results.is_empty());
        for result in &results.results {
            assert!(!result.snippet.is_empty());
        }
    }

    #[tokio::test]
    async fn search_pagination() {
        let core = create_core();

        let all = core.search_notes("", None, None, 100, 0).await.unwrap();
        let page1 = core.search_notes("", None, None, 2, 0).await.unwrap();
        let page2 = core.search_notes("", None, None, 2, 2).await.unwrap();

        assert_eq!(page1.total, all.total);
        assert_eq!(page2.total, all.total);
        assert_eq!(page1.results.len(), 2);
    }
}

// =============================================================================
// List Notes Functionality
// =============================================================================

mod list_notes {
    use super::*;

    #[tokio::test]
    async fn list_non_recursive() {
        let core = create_core();

        let list = core
            .list_notes(None, false, None, None, 100, 0)
            .await
            .unwrap();

        for note in &list.notes {
            assert!(
                !note.path.contains('/'),
                "path {} should be in root",
                note.path
            );
        }
    }

    #[tokio::test]
    async fn list_recursive() {
        let core = create_core();

        let list = core
            .list_notes(None, true, None, None, 100, 0)
            .await
            .unwrap();

        assert!(list.total >= 7);
    }

    #[tokio::test]
    async fn list_sorted_by_title() {
        let core = create_core();

        let list = core
            .list_notes(None, true, None, Some("title"), 100, 0)
            .await
            .unwrap();

        let titles: Vec<_> = list.notes.iter().map(|n| &n.title).collect();
        let mut sorted = titles.clone();
        sorted.sort();
        assert_eq!(titles, sorted);
    }

    #[tokio::test]
    async fn list_includes_word_count() {
        let core = create_core();

        let list = core
            .list_notes(Some(&folder("wiki")), true, None, None, 100, 0)
            .await
            .unwrap();

        for note in &list.notes {
            assert!(note.word_count > 0);
        }
    }
}

// =============================================================================
// Nested Folders
// =============================================================================

mod nested_folders {
    use super::*;

    #[tokio::test]
    async fn lists_nested_folder_structure() {
        let server = create_server();

        let result = server
            .read_resource_by_uri("tarn://vault/folders")
            .await
            .unwrap();
        let folders = parse_resource_result(&result);

        let paths: Vec<&str> = folders["folders"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f["path"].as_str().unwrap())
            .collect();

        // Check nested project folders
        assert!(paths.iter().any(|p| p.contains("webapp/design")));
        assert!(paths.iter().any(|p| p.contains("webapp/development")));
        assert!(paths.iter().any(|p| p.contains("webapp/docs")));

        // Check nested wiki folders
        assert!(paths.iter().any(|p| p.contains("programming/rust")));
        assert!(paths.iter().any(|p| p.contains("programming/web")));

        // Check nested areas folders
        assert!(paths.iter().any(|p| p.contains("areas/work")));
        assert!(paths.iter().any(|p| p.contains("areas/personal")));
        assert!(paths.iter().any(|p| p.contains("personal/health")));
    }

    #[tokio::test]
    async fn reads_deeply_nested_note() {
        let server = create_server();

        // 3 levels deep: areas/personal/health/Fitness Plan.md
        let result = server
            .read_resource_by_uri("tarn://note/areas/personal/health/Fitness Plan.md")
            .await
            .unwrap();
        let note = parse_resource_result(&result);

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
    async fn reads_nested_project_note() {
        let server = create_server();

        let result = server
            .read_resource_by_uri("tarn://note/projects/webapp/development/API Design.md")
            .await
            .unwrap();
        let note = parse_resource_result(&result);

        assert_eq!(note["title"], "API Design");
        assert!(
            note["tags"]
                .as_array()
                .unwrap()
                .iter()
                .any(|t| t == "project/webapp")
        );
    }

    #[tokio::test]
    async fn vault_info_scoped_to_nested_folder() {
        let server = create_server();

        let result = server
            .read_resource_by_uri("tarn://vault/info/projects/webapp")
            .await
            .unwrap();
        let info = parse_resource_result(&result);

        assert_eq!(info["folder"], "projects/webapp/");
        // Should include notes from all subfolders: design (2) + development (2) + docs (1)
        assert_eq!(info["note_count"].as_u64().unwrap(), 5);
    }

    #[tokio::test]
    async fn vault_info_deeply_nested() {
        let server = create_server();

        let result = server
            .read_resource_by_uri("tarn://vault/info/areas/personal/health")
            .await
            .unwrap();
        let info = parse_resource_result(&result);

        assert_eq!(info["folder"], "areas/personal/health/");
        assert_eq!(info["note_count"].as_u64().unwrap(), 1);
    }

    #[tokio::test]
    async fn list_notes_recursive_in_nested_folder() {
        let core = create_core();

        // List all notes under projects/webapp recursively
        let list = core
            .list_notes(Some(&folder("projects/webapp")), true, None, None, 100, 0)
            .await
            .unwrap();

        assert_eq!(list.total, 5);

        let paths: Vec<&str> = list.notes.iter().map(|n| n.path.as_str()).collect();
        assert!(paths.iter().any(|p| p.contains("design")));
        assert!(paths.iter().any(|p| p.contains("development")));
        assert!(paths.iter().any(|p| p.contains("docs")));
    }

    #[tokio::test]
    async fn list_notes_non_recursive_in_nested_folder() {
        let core = create_core();

        // List only notes directly in projects/webapp/design (not subfolders)
        let list = core
            .list_notes(
                Some(&folder("projects/webapp/design")),
                false,
                None,
                None,
                100,
                0,
            )
            .await
            .unwrap();

        assert_eq!(list.total, 2);

        for note in &list.notes {
            assert!(note.path.starts_with("projects/webapp/design/"));
            // Should not have additional path segments
            let remaining = note.path.strip_prefix("projects/webapp/design/").unwrap();
            assert!(!remaining.contains('/'));
        }
    }

    #[tokio::test]
    async fn search_in_nested_folder() {
        let core = create_core();

        // Search within nested folder
        let results = core
            .search_notes("API", Some(&folder("projects/webapp")), None, 100, 0)
            .await
            .unwrap();

        assert!(results.total >= 1);
        for result in &results.results {
            assert!(result.path.starts_with("projects/webapp/"));
        }
    }

    #[tokio::test]
    async fn tags_scoped_to_nested_folder() {
        let server = create_server();

        let result = server
            .read_resource_by_uri("tarn://vault/tags/projects/webapp")
            .await
            .unwrap();
        let tags = parse_resource_result(&result);

        assert_eq!(tags["folder"], "projects/webapp/");

        let tag_names: Vec<&str> = tags["tags"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["tag"].as_str().unwrap())
            .collect();

        // Should have project/webapp tag from nested notes
        assert!(tag_names.contains(&"project/webapp"));
    }

    #[tokio::test]
    async fn list_wiki_nested_programming_notes() {
        let core = create_core();

        let list = core
            .list_notes(Some(&folder("wiki/programming")), true, None, None, 100, 0)
            .await
            .unwrap();

        // rust (2) + web (1) = 3
        assert_eq!(list.total, 3);

        let paths: Vec<&str> = list.notes.iter().map(|n| n.path.as_str()).collect();
        assert!(paths.iter().any(|p| p.contains("Async Rust")));
        assert!(paths.iter().any(|p| p.contains("Error Handling")));
        assert!(paths.iter().any(|p| p.contains("WebSockets")));
    }

    #[tokio::test]
    async fn cross_folder_links_work() {
        let core = create_core();

        // Read a nested note that links to another nested note
        let note = core
            .read_note(
                "projects/webapp/design/UI Mockups.md",
                None,
                false,
                true,
                false,
            )
            .await
            .unwrap();

        let links = note.links.unwrap();
        let targets: Vec<&str> = links.iter().map(|l| l.target.as_str()).collect();

        // Should link to Color Palette in same folder
        assert!(targets.iter().any(|t| t.contains("Color Palette")));
        // Should link to parent WebApp
        assert!(targets.iter().any(|t| t.contains("WebApp")));
    }

    #[tokio::test]
    async fn folders_resource_shows_nested_counts() {
        let server = create_server();

        let result = server
            .read_resource_by_uri("tarn://vault/folders/projects")
            .await
            .unwrap();
        let folders = parse_resource_result(&result);

        let folder_list = folders["folders"].as_array().unwrap();

        // Find the design folder and check its count
        let design = folder_list
            .iter()
            .find(|f| f["path"].as_str().unwrap().contains("design"));
        assert!(design.is_some());
        assert_eq!(design.unwrap()["note_count"].as_u64().unwrap(), 2);
    }
}
