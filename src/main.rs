use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use clap::{Args, Parser, ValueEnum};
use rmcp::ServiceExt;
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService,
    session::local::{LocalSessionManager, SessionConfig},
};
use tokio::task::JoinHandle;

use tarn::TarnBuilder;
use tarn::index::IndexConfig;
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

    /// Enable in-memory index for fast search
    #[arg(long)]
    index: bool,

    /// Path for persistent index storage (implies --index)
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

    let use_index = cli.index || cli.index_path.is_some();

    let (core, _index_sync_handle): (Arc<_>, Option<JoinHandle<()>>) = if use_index {
        let mut builder = if let Some(vault) = cli.vault {
            TarnBuilder::local(vault)
        } else {
            TarnBuilder::from_env()?
        };

        let index_config = IndexConfig::default();
        builder = if let Some(index_path) = cli.index_path {
            builder.with_persistent_index(index_config, index_path)
        } else {
            builder.with_index(index_config)
        };

        let core = Arc::new(builder.build_async().await?);

        tracing::info!("rebuilding index...");
        core.rebuild_index().await?;
        tracing::info!("index rebuilt");

        let handle = core.start_index_sync()?;
        tracing::info!("index sync started");

        (core, Some(handle))
    } else {
        let core = if let Some(vault) = cli.vault {
            Arc::new(TarnBuilder::local(vault).build())
        } else {
            Arc::new(TarnBuilder::from_env()?.build())
        };
        (core, None)
    };

    match cli.transport {
        Transport::Stdio => {
            let server = TarnMcpServer::new(core);
            let service = server.serve(rmcp::transport::stdio()).await?;
            service.waiting().await?;
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

    Ok(())
}
