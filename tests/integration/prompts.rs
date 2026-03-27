use crate::common::*;
use serde_json::json;

#[tokio::test]
async fn explore_topic_generates_messages() {
    let (_tmp, client) = spawn_server(false).await;

    let result = get_prompt(&client, "tarn_explore_topic", json!({"topic": "ownership"})).await;

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
    let (_tmp, client) = spawn_server(false).await;

    let result = get_prompt(
        &client,
        "tarn_explore_topic",
        json!({"topic": "Rust", "folder": "wiki"}),
    )
    .await;

    assert!(result.description.unwrap().contains("Rust"));
}

#[tokio::test]
async fn explore_topic_missing_arg_returns_error() {
    let (_tmp, client) = spawn_server(false).await;

    let result = client
        .get_prompt(rmcp::model::GetPromptRequestParams::new(
            "tarn_explore_topic",
        ))
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn summarize_project_generates_messages() {
    let (_tmp, client) = spawn_server(false).await;

    let result = get_prompt(
        &client,
        "tarn_summarize_project",
        json!({"folder": "projects"}),
    )
    .await;

    assert!(result.description.unwrap().contains("projects"));
    assert!(result.messages.len() >= 2);
}

#[tokio::test]
async fn summarize_project_missing_arg_returns_error() {
    let (_tmp, client) = spawn_server(false).await;

    let result = client
        .get_prompt(rmcp::model::GetPromptRequestParams::new(
            "tarn_summarize_project",
        ))
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn unknown_prompt_returns_error() {
    let (_tmp, client) = spawn_server(false).await;

    let result = client
        .get_prompt(rmcp::model::GetPromptRequestParams::new("unknown_prompt"))
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn lists_all_prompts() {
    let (_tmp, client) = spawn_server(false).await;

    let prompts = client.list_all_prompts().await.unwrap();
    let names: Vec<&str> = prompts.iter().map(|p| p.name.as_str()).collect();

    assert!(names.contains(&"tarn_explore_topic"));
    assert!(names.contains(&"tarn_summarize_project"));
    assert_eq!(names.len(), 2);
}
