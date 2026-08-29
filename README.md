# tarn

> *A tarn is a small mountain lake formed in a glacial cirque — deep, still, and hidden in rocky highland terrain. From
Old Norse **tjörn**.*

Tarn is a Rust library and [MCP (Model Context Protocol)](https://modelcontextprotocol.io) server that exposes markdown
knowledge vaults ([Obsidian](https://obsidian.md)-compatible) to AI agents. It indexes at the **section** level —
heading-delimited passages carrying a full heading path — so an agent retrieves the relevant passage rather than the
whole file.

## Features

- **Section-level retrieval** — search returns ranked sections with their heading path, addressable directly as
  `tarn://note/{path}#{section_path}`
- **Obsidian-aware parsing** — wikilinks (`[[note]]`, `[[note|alias]]`, `[[note#heading]]`), frontmatter (YAML), inline
  tags (`#tag`, `#nested/tag`), embeds (`![[image.png]]`)
- **Hybrid ranking** — BM25 over stemmed tokens (8 languages, auto-detected) fused with tag similarity via reciprocal
  rank fusion
- **MCP tools** — search, tag queries, and six write operations
- **MCP resources** — vault info, tag hierarchy, folder structure, notes, and note sections
- **MCP prompts** — guided workflows for topic exploration and project summarization
- **Dual transport** — stdio (for Claude Desktop) or HTTP (Streamable HTTP with SSE)
- **Server-tracked revisions** — optimistic concurrency control for safe concurrent writes, with no revision token for
  the agent to carry
- **Measured retrieval** — `make bench` scores the shipped binary against BEIR relevance judgments, so a ranking change
  moves a number instead of resting on judgement

## Installation

### Pre-built binaries

Download the latest release for your platform from [GitHub Releases](https://github.com/AlKuz/tarn/releases):

| Platform | Architecture             | Binary                     |
|----------|--------------------------|----------------------------|
| macOS    | Apple Silicon (M1/M2/M3) | `tarn-mcp-darwin-arm64`    |
| macOS    | Intel                    | `tarn-mcp-darwin-x64`      |
| Linux    | x86_64                   | `tarn-mcp-linux-x64`       |
| Linux    | ARM64                    | `tarn-mcp-linux-arm64`     |
| Windows  | x86_64                   | `tarn-mcp-windows-x64.exe` |

**macOS / Linux:**

```bash
# Download (replace URL with your platform)
curl -LO https://github.com/AlKuz/tarn/releases/latest/download/tarn-mcp-darwin-arm64

# Make executable
chmod +x tarn-mcp-darwin-arm64

# Move to PATH
sudo mv tarn-mcp-darwin-arm64 /usr/local/bin/tarn-mcp
```

**Windows (PowerShell):**

```powershell
# Download
Invoke-WebRequest -Uri https://github.com/AlKuz/tarn/releases/latest/download/tarn-mcp-windows-x64.exe -OutFile tarn-mcp.exe

# Move to a directory in your PATH
Move-Item tarn-mcp.exe C:\Windows\System32\
```

### From source

```bash
cargo install --path .
```

Or build manually:

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
            "args": [
                "--vault",
                "/path/to/your/obsidian/vault"
            ]
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

```text
tarn-mcp [OPTIONS]

Options:
    --transport <TRANSPORT>      Transport protocol [default: stdio] [possible values: stdio, http]
    --vault <VAULT>              Vault path (overrides STORAGE__PATH env var)
    --index-path <INDEX_PATH>    State directory for the index and revision tracker
                                 [default: <data-local-dir>/tarn/<hash of vault path>]
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

| Tool                     | Description                                                                          |
|--------------------------|--------------------------------------------------------------------------------------|
| `tarn_search_notes`      | Search the vault; returns notes ranked by relevance with section scores. Supports `tag:` and `folder:` inline filters, and `rendered=true` for markdown output |
| `tarn_get_tags`          | Get tag hierarchy with usage statistics                                              |
| `tarn_create_note`       | Create a new note; fails if one already exists at the path                           |
| `tarn_update_note`       | Replace or append note content, with a server-side revision check                    |
| `tarn_replace_in_note`   | Replace text within a note (`first`, `all`, or `regex` mode)                          |
| `tarn_update_frontmatter`| Modify frontmatter keys without rewriting content                                    |
| `tarn_delete_note`       | Delete a note                                                                        |
| `tarn_rename_note`       | Rename or move a note, updating wikilinks in other notes by default                  |

### Resources

| URI                                  | Description                                            |
|--------------------------------------|--------------------------------------------------------|
| `tarn://vault/info`                  | Vault metadata (name, note count, tag count)           |
| `tarn://vault/tags`                  | Tag hierarchy with counts                              |
| `tarn://vault/folders`               | Directory structure with note counts                   |
| `tarn://vault/info/{folder}`         | Vault metadata scoped to a folder subtree              |
| `tarn://vault/tags/{folder}`         | Tag hierarchy scoped to a folder subtree               |
| `tarn://vault/folders/{folder}`      | Directory tree scoped to a folder subtree              |
| `tarn://note/{path}`                 | Individual note content and metadata                   |
| `tarn://note/{path}#{section_path}`  | Section content by heading path (e.g. `Design/API`)    |

### Prompts

| Prompt                   | Description                                    |
|--------------------------|------------------------------------------------|
| `tarn_explore_topic`     | Guided deep-dive into a topic across the vault |
| `tarn_summarize_project` | Generate project status summary from a folder  |

## Architecture

`tarn` is a library crate; `tarn-mcp` (`src/main.rs`) is a thin CLI wrapper around it. The library exposes four ports
behind traits — `Storage`, `Index`, `Observer`, `RevisionTracker` — composed by the `TarnCore` facade.

```text
src/
├── main.rs        # tarn-mcp binary: CLI, tracing, transport wiring
├── lib.rs         # public API
├── common/        # VaultPath, RevisionToken, DataURI, Buildable/Configurable/Persistable
├── note_handler/  # Obsidian markdown parsing (frontmatter, sections, links, tags, tasks)
├── tokenizer/     # Tokenizer port: naive, stemming, ngram, hf (feature-gated)
├── storage/       # Storage port + LocalStorage
├── observer/      # Observer port + filesystem watcher
├── revisions/     # RevisionTracker port + in-memory tracker
├── index/         # Index port + InMemoryIndex (bm25, tags, rrf, scorer)
├── core/          # TarnCore facade + TarnConfig
└── mcp/           # TarnMcpServer: tools, resources, prompts, sync
```

Alongside it, `scripts/bench/` holds the retrieval evaluation harness (Python, driven by `make bench`).

See [docs/architecture/01_introduction_and_goals.md](docs/architecture/01_introduction_and_goals.md) for the full
building-block, runtime and deployment views.

## Benchmarks

Retrieval quality is measured, not asserted. `make bench` downloads a [BEIR](https://github.com/beir-cellar/beir)
corpus, converts it into a Tarn vault, spawns the release binary and queries it over MCP/stdio, then scores the
hits against human relevance judgments with `pytrec_eval`.

```bash
make bench                      # Full run on scifact: download, index, search, score, report
make bench cmd=list             # The ten-dataset catalogue with tiers and sizes
```

### Choosing datasets

`dataset` takes one name, several, or `all`.

```bash
make bench dataset=nfcorpus                      # One corpus
make bench dataset="scifact nfcorpus arguana"    # Several
make bench dataset=all                           # The whole catalogue
```

`make bench cmd=list` prints the ten-corpus catalogue with sizes. Which of them are worth running — and
which will not finish, because cold indexing is O(N²) in vault size — is in
[scripts/bench/README.md](scripts/bench/README.md#datasets). `dataset=all` will not complete today.

A multi-dataset run does the whole pipeline per dataset rather than each stage across all of them, so an
interrupted batch still leaves complete, scored results for what it finished. One dataset failing does not
abort the rest — the failures are collected, reported at the end, and the exit code is non-zero.

### Configuration

These are the knobs that change what search returns. Runs are grouped by exactly this set, so changing one
and re-running produces a side-by-side comparison in the version report rather than overwriting the previous
number.

| Variable | Default | What it does |
|---|---|---|
| `FEATURES` | `stemming` | Cargo features the release binary is built with; `none` for no features at all. Changes tokenization, and so every ranking score. |
| `TOP_K` | `100` | Sections requested per query. Tarn's own default is 20, which is too shallow to fill a k=100 cutoff. Caps sections *before* they are grouped into notes. |
| `TOKEN_LIMIT` | *(unset)* | Cumulative token budget over the returned sections, applied after `TOP_K`. Unset means no budget. Set it to measure what a context budget costs in recall. |
| `SCORE_THRESHOLD` | `0.0` | Minimum fused score. Measured on the reciprocal-rank scale, which caps near `0.0328` — despite the tool describing it as 0.0–1.0. Anything higher returns nothing. |
| `COLD` | *(unset)* | Non-empty rebuilds the index from scratch instead of reusing persisted state. Off by default because rebuilding costs minutes and the ranking metrics are identical either way. |

```bash
make bench TOP_K=20                      # What Tarn's own default costs in recall
make bench TOKEN_LIMIT=4000              # What a context budget costs
make bench FEATURES=none COLD=1          # Does stemming earn its keep, from a clean index
```

Each of those lands as a new row in the version report's comparison table, best nDCG@10 first.

`FEATURES` is passed to `cargo build`, so a full `make bench` run measures what it claims. The `cmd=search`
stage does not rebuild, so there `FEATURES` only labels the manifest — build first if you change it.

Results land in `target/benchmarks/`:

```text
report.md                        # entry point: most recent run per dataset, a row per version
0.9.1/
  report.md                      # one version in full: configurations compared, history, detail
  runs/<run-id>/                 # raw artifacts, with a manifest pinning build, features, corpus
  state/<features>/<dataset>/    # the index tarn persisted, keyed so builds cannot share one
```

Version is the top-level partition: everything a build produced sits together, with its report at the top.
Within a version, runs are grouped by configuration, so `TOP_K=20` against `TOP_K=100` is a table rather than
an archaeology exercise. Index state is keyed by version and features too, so two builds can never share one
index — Tarn records nothing about what built an index, so a shared directory would silently reuse a stale
one. Datasets are downloaded into `data/`. Both trees are gitignored.

### Individual stages

For iterating without re-running the pipeline. Each honours `dataset` and the configuration variables above.

```bash
make bench cmd=setup            # Install the Python dependencies (uv) only
make bench cmd=download         # Fetch the corpus
make bench cmd=adapt            # Build the vault and eval manifest
make bench cmd=search           # Query the binary
make bench cmd=score            # nDCG, Recall, Precision, MRR, MAP
make bench cmd=ragas            # Text-level cross-check
make bench cmd=report           # Rebuild report.md from existing runs
make bench cmd=clean            # Remove data/ and target/benchmarks/
```

`download` and `adapt` skip a dataset they have already prepared, so repeating a run costs nothing
for the corpus. To rebuild one dataset, delete `data/vault/<name>` or `data/eval/<name>`.

Requires [uv](https://docs.astral.sh/uv/); nothing else in this repository does, and `make build`, `test`, `lint`
and `ci` are untouched by it. Note that `make clean` runs `cargo clean` and so removes benchmark history along
with `target/` — use `make bench cmd=clean` when you mean the bench.

See [scripts/bench/README.md](scripts/bench/README.md) for the datasets, the methodology, and what each metric
tells you, and [ADR-0016](docs/adr/0016-retrieval-eval-bench.md) for why the bench is shaped this way.

## Development

```bash
make help              # Show all available commands
```

### Build

```bash
make build             # Debug build
make build cmd=release # Release build
make build cmd=check   # Type-check only
```

### Test

```bash
make test                    # Run all tests
make test cmd=unit           # Unit tests only
make test cmd=integration    # Integration tests only
make test cmd=verbose        # Tests with output
```

### Lint & Format

```bash
make lint              # Check formatting and run clippy
make lint cmd=fix      # Auto-fix issues
make lint cmd=fmt      # Format code only
```

### Coverage

Requires `cargo-llvm-cov`:

```bash
cargo install cargo-llvm-cov   # Install coverage tool

make test cmd=coverage         # HTML report (coverage/html/index.html)
```

### CI

```bash
make ci                # Full pipeline (lint, test, release build)
make ci cmd=quick      # Quick check (no release build)
```

### Benchmark harness

The Python harness has its own checks, separate from the Rust pipeline:

```bash
uv sync                        # Dependencies, into .venv/
uv run mypy                    # Strict type checking
uvx ruff check scripts/bench/  # Lint
uvx ruff format scripts/bench/ # Format
```

### Debug

```bash
tarn-mcp --vault ~/Obsidian/Test --log-level debug
```

## License

MIT
