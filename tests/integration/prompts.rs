use crate::common::*;
use serde_json::json;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn explore_topic_generates_messages() {
    let server = spawn_server(false).await;

    let result = get_prompt(
        &server.client,
        "tarn_explore_topic",
        json!({"topic": "ownership"}),
    )
    .await;

    assert!(result.description.unwrap().contains("ownership"));
    assert!(!result.messages.is_empty());

    // First message should be a resource link to vault info
    let first = &result.messages[0];
    assert!(matches!(
        first.content,
        rmcp::model::PromptMessageContent::ResourceLink { .. }
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn explore_topic_with_folder() {
    let server = spawn_server(false).await;

    let result = get_prompt(
        &server.client,
        "tarn_explore_topic",
        json!({"topic": "Rust", "folder": "wiki"}),
    )
    .await;

    assert!(result.description.unwrap().contains("Rust"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn explore_topic_missing_arg_returns_error() {
    let server = spawn_server(false).await;

    let result = server
        .client
        .get_prompt(rmcp::model::GetPromptRequestParams::new(
            "tarn_explore_topic",
        ))
        .await;
    assert!(result.is_err());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn summarize_project_generates_messages() {
    let server = spawn_server(false).await;

    let result = get_prompt(
        &server.client,
        "tarn_summarize_project",
        json!({"folder": "projects"}),
    )
    .await;

    assert!(result.description.unwrap().contains("projects"));
    assert!(result.messages.len() >= 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn summarize_project_missing_arg_returns_error() {
    let server = spawn_server(false).await;

    let result = server
        .client
        .get_prompt(rmcp::model::GetPromptRequestParams::new(
            "tarn_summarize_project",
        ))
        .await;
    assert!(result.is_err());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unknown_prompt_returns_error() {
    let server = spawn_server(false).await;

    let result = server
        .client
        .get_prompt(rmcp::model::GetPromptRequestParams::new("unknown_prompt"))
        .await;
    assert!(result.is_err());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn lists_all_prompts() {
    let server = spawn_server(false).await;

    let prompts = server.client.list_all_prompts().await.unwrap();
    let names: Vec<&str> = prompts.iter().map(|p| p.name.as_str()).collect();

    assert!(names.contains(&"tarn_explore_topic"));
    assert!(names.contains(&"tarn_summarize_project"));
    assert_eq!(names.len(), 2);
}
