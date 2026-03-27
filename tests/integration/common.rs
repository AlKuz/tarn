use std::path::{Path, PathBuf};

use rmcp::ServiceExt;
use rmcp::model::*;
use rmcp::service::RunningService;
use rmcp::transport::{ConfigureCommandExt, TokioChildProcess};
use serde_json::Value;

pub type Client = RunningService<rmcp::RoleClient, ()>;

pub fn binary_path() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_tarn-mcp"))
}

pub fn vault_source() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/vault")
}

pub fn copy_dir_all(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).unwrap();
    for entry in std::fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let ty = entry.file_type().unwrap();
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dst.join(entry.file_name()));
        } else {
            std::fs::copy(entry.path(), dst.join(entry.file_name())).unwrap();
        }
    }
}

pub async fn spawn_server(with_index: bool) -> (tempfile::TempDir, Client) {
    let tmp = tempfile::tempdir().unwrap();
    let vault = tmp.path().join("vault");
    copy_dir_all(&vault_source(), &vault);

    let transport = TokioChildProcess::new(tokio::process::Command::new(binary_path()).configure(
        |cmd| {
            cmd.arg("--vault")
                .arg(&vault)
                .arg("--log-level")
                .arg("warn");
            if with_index {
                cmd.arg("--index");
            }
        },
    ))
    .unwrap();

    let client = ().serve(transport).await.unwrap();
    (tmp, client)
}

/// Call a tool and parse the JSON response. Panics on tool errors.
pub async fn call_tool(client: &Client, name: &str, args: Value) -> Value {
    let mut params = CallToolRequestParams::new(name.to_string());
    if let Value::Object(map) = args {
        params = params.with_arguments(map);
    }
    let result = client.call_tool(params).await.unwrap();
    assert!(
        !result.is_error.unwrap_or(false),
        "tool {name} returned error: {}",
        result.content[0]
            .as_text()
            .map(|t| t.text.as_str())
            .unwrap_or("?")
    );
    let text = &result.content[0].as_text().unwrap().text;
    serde_json::from_str(text).unwrap()
}

/// Call a tool expecting an error. Returns the error text.
pub async fn call_tool_expect_error(client: &Client, name: &str, args: Value) -> String {
    let mut params = CallToolRequestParams::new(name.to_string());
    if let Value::Object(map) = args {
        params = params.with_arguments(map);
    }
    let result = client.call_tool(params).await.unwrap();
    assert!(
        result.is_error.unwrap_or(false),
        "expected tool error but got success"
    );
    result.content[0]
        .as_text()
        .map(|t| t.text.clone())
        .unwrap_or_default()
}

/// Read a resource and parse the JSON response.
pub async fn read_resource(client: &Client, uri: &str) -> Value {
    let result = client
        .read_resource(ReadResourceRequestParams::new(uri))
        .await
        .unwrap();
    match &result.contents[0] {
        ResourceContents::TextResourceContents { text, .. } => serde_json::from_str(text).unwrap(),
        _ => panic!("expected text resource"),
    }
}

/// Get a prompt result.
pub async fn get_prompt(client: &Client, name: &str, args: Value) -> GetPromptResult {
    let mut params = GetPromptRequestParams::new(name);
    if let Value::Object(map) = args {
        params = params.with_arguments(map);
    }
    client.get_prompt(params).await.unwrap()
}
