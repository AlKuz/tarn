# tarn

> *A tarn is a small mountain lake formed in a glacial cirque — deep, still, and hidden in rocky highland terrain. From Old Norse **tjörn**.*

Tarn is an [MCP (Model Context Protocol)](https://modelcontextprotocol.io) server that exposes [Obsidian](https://obsidian.md) vaults to AI agents. It parses markdown notes with full Obsidian syntax support — wikilinks, frontmatter, tags, embeds — and provides tools for searching, listing, and reading your knowledge base.

## Features

- **Obsidian-aware parsing** — wikilinks (`[[note]]`, `[[note|alias]]`, `[[note#heading]]`), frontmatter (YAML), inline tags (`#tag`, `#nested/tag`), embeds (`![[image.png]]`)
- **MCP tools** — `tarn_read_note`, `tarn_search_notes`, `tarn_list_notes`, `tarn_get_tags`
- **MCP resources** — vault info, tag hierarchy, folder structure
- **MCP prompts** — guided workflows for topic exploration and project summarization
- **Dual transport** — stdio (for Claude Desktop) or HTTP (Streamable HTTP with SSE)
- **Revision tokens** — optimistic concurrency control for safe writes

## Installation

```bash
cargo install --path .
```

Or build from source:

```bash
cargo build --release
./target/release/tarn-mcp --help
```

## Usage

### Claude Desktop (stdio)

Add to your Claude Desktop config (`~/Library/Application Support/Claude/claude_desktop_config.json`):

```json
{
  "mcpServers": {
    "tarn": {
      "command": "tarn-mcp",
      "args": ["--vault", "/path/to/your/obsidian/vault"]
    }
  }
}
```

### HTTP Server

```bash
tarn-mcp --transport http --vault ~/Obsidian/MyVault --port 8000
```

The MCP endpoint will be available at `http://127.0.0.1:8000/mcp`.

### Environment Variables

Instead of `--vault`, you can set:

```bash
export STORAGE__TYPE=local
export STORAGE__PATH=/path/to/vault
tarn-mcp
```

## CLI Options

```
tarn-mcp [OPTIONS]

Options:
    --transport <TRANSPORT>      Transport protocol [default: stdio] [possible values: stdio, http]
    --vault <VAULT>              Vault path (overrides STORAGE__PATH env var)
    --log-level <LOG_LEVEL>      Log level [default: info] [possible values: trace, debug, info, warn, error]

HTTP options:
    --host <HOST>                Host address to bind [default: 127.0.0.1]
    --port <PORT>                Port to bind [default: 8000]
    --path <PATH>                MCP endpoint path [default: /mcp]
    --sse-keep-alive <SECONDS>   SSE keep-alive ping interval (0 to disable) [default: 15]
    --sse-retry <SECONDS>        SSE retry interval for client reconnection [default: 3]
    --stateless                  Disable stateful session mode
    --json-response              Use JSON responses instead of SSE (requires --stateless)
    --session-timeout <SECONDS>  Session inactivity timeout (0 for no timeout) [default: 0]
```

## MCP Capabilities

### Tools

| Tool | Description |
|------|-------------|
| `tarn_read_note` | Read note content with section filtering and summary mode |
| `tarn_search_notes` | Full-text search with tag filtering and pagination |
| `tarn_list_notes` | List notes in a folder with optional recursion |
| `tarn_get_tags` | Get tag hierarchy with usage counts |

### Resources

| URI | Description |
|-----|-------------|
| `tarn://vault/info` | Vault metadata (name, note count, tag count) |
| `tarn://vault/tags` | Tag hierarchy with counts |
| `tarn://vault/folders` | Directory structure with note counts |
| `tarn://note/{path}` | Individual note content and metadata |

### Prompts

| Prompt | Description |
|--------|-------------|
| `tarn_explore_topic` | Guided deep-dive into a topic across the vault |
| `tarn_summarize_project` | Generate project status summary from a folder |

## Architecture

```
src/
├── main.rs           # CLI and MCP server entry point
├── lib.rs            # Public API
├── core/
│   ├── builder.rs    # TarnCore builder pattern
│   ├── tarn_core.rs  # Core business logic
│   ├── config.rs     # Configuration from env
│   ├── storage/      # Storage abstraction (local filesystem)
│   ├── parser/       # Markdown parsing (frontmatter, links, tags, sections)
│   └── common/       # Shared types (RevisionToken, DataURI)
└── mcp/
    ├── mod.rs        # MCP server handler
    ├── tools.rs      # Tool implementations
    ├── resources.rs  # Resource handlers
    └── prompts.rs    # Prompt templates
```

## Development

```bash
# Run tests
cargo test

# Run with debug logging
tarn-mcp --vault ~/Obsidian/Test --log-level debug

# Check for issues
cargo clippy
```

See [`docs/development-plan.md`](./docs/development-plan.md) for planned features and optimizations.

## License

MIT
