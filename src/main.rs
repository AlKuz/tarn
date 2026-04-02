use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use clap::{Args, Parser, ValueEnum};
use rmcp::ServiceExt;
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService,
    session::local::{LocalSessionManager, SessionConfig},
};
use tarn::TarnConfig;
use tarn::common::Buildable;
use tarn::index::{InMemoryIndexConfig, IndexConfig};
use tarn::mcp::TarnMcpServer;

#[derive(Clone, ValueEnum)]
enum Transport {
    Stdio,
    Http,
}

#[derive(Clone, ValueEnum)]
enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    fn as_filter(&self) -> &'static str {
        match self {
            LogLevel::Trace => "trace",
            LogLevel::Debug => "debug",
            LogLevel::Info => "info",
            LogLevel::Warn => "warn",
            LogLevel::Error => "error",
        }
    }
}

#[derive(Args)]
struct HttpOptions {
    /// Host address to bind
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    /// Port to bind
    #[arg(long, default_value_t = 8000)]
    port: u16,

    /// MCP endpoint path
    #[arg(long, default_value = "/mcp")]
    path: String,

    /// SSE keep-alive ping interval in seconds (0 to disable)
    #[arg(long, default_value_t = 15)]
    sse_keep_alive: u64,

    /// SSE retry interval in seconds for client reconnection
    #[arg(long, default_value_t = 3)]
    sse_retry: u64,

    /// Disable stateful session mode (sessions won't persist across requests)
    #[arg(long)]
    stateless: bool,

    /// Use JSON responses instead of SSE for simple request-response (requires --stateless)
    #[arg(long)]
    json_response: bool,

    /// Session inactivity timeout in seconds (0 for no timeout)
    #[arg(long, default_value_t = 0)]
    session_timeout: u64,
}

#[derive(Parser)]
#[command(name = "tarn-mcp", about = "Tarn MCP server for Obsidian vaults")]
struct Cli {
    /// Transport protocol
    #[arg(long, default_value = "stdio")]
    transport: Transport,

    /// Vault path (overrides STORAGE__PATH env var)
    #[arg(long)]
    vault: Option<PathBuf>,

    /// Log level
    #[arg(long, default_value = "info")]
    log_level: LogLevel,

    /// Override the default index persistence path
    #[arg(long)]
    index_path: Option<PathBuf>,

    /// HTTP transport options
    #[command(flatten)]
    http: HttpOptions,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(cli.log_level.as_filter())
        .init();

    let mut config = if let Some(vault) = cli.vault {
        TarnConfig::local(vault)
    } else {
        TarnConfig::from_env()?
    };

    // Override index persistence path if specified
    if let Some(index_path) = cli.index_path {
        config = config.with_index(IndexConfig::InMemory(InMemoryIndexConfig {
            persistence_path: Some(index_path),
            ..Default::default()
        }));
    }

    let core = Arc::new(config.build()?);

    tracing::info!("rebuilding index...");
    core.rebuild_index().await?;
    tracing::info!("index rebuilt");

    let index_sync_handle = core.start_index_sync();
    tracing::info!("index sync started");

    match cli.transport {
        Transport::Stdio => {
            let server = TarnMcpServer::new(core);
            let service = server.serve(rmcp::transport::stdio()).await?;
            service.waiting().await?;
            index_sync_handle.abort();
        }
        Transport::Http => {
            let ct = tokio_util::sync::CancellationToken::new();

            let session_config = SessionConfig {
                keep_alive: if cli.http.session_timeout > 0 {
                    Some(Duration::from_secs(cli.http.session_timeout))
                } else {
                    None
                },
                ..Default::default()
            };

            let session_manager = LocalSessionManager {
                session_config,
                ..Default::default()
            };

            let server_config = StreamableHttpServerConfig {
                sse_keep_alive: if cli.http.sse_keep_alive > 0 {
                    Some(Duration::from_secs(cli.http.sse_keep_alive))
                } else {
                    None
                },
                sse_retry: Some(Duration::from_secs(cli.http.sse_retry)),
                stateful_mode: !cli.http.stateless,
                json_response: cli.http.json_response,
                cancellation_token: ct.child_token(),
            };

            let service = StreamableHttpService::new(
                move || Ok(TarnMcpServer::new(core.clone())),
                session_manager.into(),
                server_config,
            );

            let router = axum::Router::new().nest_service(&cli.http.path, service);
            let addr = format!("{}:{}", cli.http.host, cli.http.port);
            let listener = tokio::net::TcpListener::bind(&addr).await?;
            tracing::info!("MCP HTTP server at http://{addr}{}", cli.http.path);

            axum::serve(listener, router)
                .with_graceful_shutdown(async move {
                    tokio::signal::ctrl_c().await.unwrap();
                    ct.cancel();
                })
                .await?;
        }
    }

    index_sync_handle.abort();
    Ok(())
}
