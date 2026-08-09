# Competitors

Competitive intelligence for **Tarn**, as it pivots from an Obsidian-only MCP server to
general-purpose local context serving.

**55 projects profiled**, all versions verified against the GitHub API and package
registries on **2026-08-09**. 18 were read at source level (**bold**); the rest are
web-research profiles.

* **[analysis.md](analysis.md)** — cross-cutting analysis: what is now commodity, what ground
  is unoccupied, corrections to the 2026-03 review, and Tarn's revised positioning. **Start
  here.**
* Per-project profiles are grouped by category below.

Legend: 🟢 active · 🟡 dormant · 🔴 abandoned · ⚫ archived · **bold** = deep dive (source read)

### The short version

* Rust + BM25 + persistent index + MCP is the **category baseline**, not a differentiator —
  [codesearch](code/codesearch.md) says so in its own README.
* Heading-aware chunking is commodity too; **section-level *ranking* is not**.
  [engraph](general/engraph.md) chunks by section and then fuses at file granularity.
* [lore](general/lore.md) already occupies Tarn's target architecture at v0.1.0 and ~17 stars;
  [engraph](general/engraph.md) holds Tarn's current ground and is ahead on ranking and
  distribution.
* Every document competitor **degrades to fixed windows on non-markdown** — the pivot's main
  opening, and [xberg](general/xberg.md) makes it reachable.
* The real baseline is [filesystem MCP + the agent's own grep](general/mcp-reference-servers.md),
  at ~89k stars and zero setup.

## General-purpose

Mixed local corpora, universal context, and agent memory — the category Tarn is pivoting into. — [category index](general/index.md)

* 🟡 [**engraph**](general/engraph.md) `1.7.2` — Rust MCP server over Obsidian vaults with five-lane RRF hybrid search, an LLM query orchestrator, 25 tools, and a REST API.
* 🟢 [**lore**](general/lore.md) `0.1.0` — Rust MCP server and CLI that ingests 90+ document formats from 10 source types into a local Tantivy BM25 index with heading-aware chunking.
* 🟢 [**ostk-recall**](general/ostk-recall.md) `0.9.3` — Rust multi-corpus MCP retrieval daemon fusing model2vec vectors with Tantivy BM25 over notes, code, and agent session logs behind two tools.
* 🟢 [**Xberg (formerly Kreuzberg)**](general/xberg.md) `1.1.0` — Rust-core polyglot document intelligence framework extracting 101 formats into a heading-nested DocumentStructure — a candidate dependency for Tarn's pivot, and also an MCP server in its own right.
* 🟢 [Agent-memory tier (mem0, Graphiti/Zep, Letta)](general/agent-memory-tier.md) `various` — The 100k-star memory category that indexes what the agent said rather than what you wrote — not competitors, but a serious category-confusion risk.
* 🟢 [basic-memory](general/basic-memory.md) `0.22.1` — AGPL Python knowledge server where the agent writes structured markdown back — typed observations and wikilink relations traversed as a graph, with a paid hosted tier over the same engine.
* 🟢 [cognee](general/cognee.md) `1.4.2` — Self-hosted knowledge-graph memory for agents exposing exactly three MCP tools while keeping its powerful internals deliberately unexposed.
* 🟢 [Khoj](general/khoj.md) `2.0.0-beta.28` — AGPL self-hostable "AI second brain" application indexing local docs and the web — the app end of the spectrum, and the shape Tarn should avoid becoming.
* 🟢 [MCP reference servers (filesystem, fetch, memory)](general/mcp-reference-servers.md) `2026.7.10` — The official baseline every agent already has — filesystem plus the agent's own grep is what Tarn must actually beat, and the honest benchmark target.
* 🟢 [Onyx (formerly Danswer)](general/onyx.md) `4.5.4` — Enterprise search over 40+ SaaS connectors with permission-aware retrieval — the team/SaaS end of the market, and the best connector abstraction in open source.
* 🟢 [Pharos (PharosRAG)](general/pharos-rag.md) `unverified` — Local-first agentic RAG over document libraries with page-level citations, and the clearest written rationale for a daemon-plus-thin-client architecture.
* 🟡 [pluck](general/pluck.md) `0.6.0` — Rust MCP code retrieval whose entire pitch is measured token savings, with every claim gated by a checked-in benchmark file and a --raw escape hatch on every tool.
* 🟡 [Rememex](general/rememex.md) `2.5.1` — Windows-only Rust local file search over 120+ file types with built-in OCR and EXIF geolocation — proof of demand for exactly Tarn's pivot, left unserved on macOS and Linux.
* 🟢 [SurfSense](general/surfsense.md) `0.0.36` — Open-source NotebookLM alternative researching the live web through one platform, API, or MCP server — web-first rather than disk-first.
* 🟢 [trusty-search](general/trusty-search.md) `0.42.3` — Rust machine-wide code search combining BM25, vectors, and a knowledge graph with zero cold-start — one always-on index across every project rather than one per repo.

## Code context

Servers that index and serve source code to agents. — [category index](code/index.md)

* 🟢 [**Codanna**](code/codanna.md) `0.13.2` — Rust MCP server indexing code symbols and call graphs across 15 languages via tree-sitter, plus a separate heading-aware document RAG collection.
* 🟢 [**codesearch (flupkede)**](code/codesearch.md) `1.2.4` — Rust MCP server with AST-aware chunking for 17 languages, arroy + Tantivy hybrid RRF retrieval, and cross-repository federated search.
* 🟢 [**Probe**](code/probe.md) `0.6.0-rc330` — Stateless Rust code search with AST-aware extraction, a boolean query language, SIMD-accelerated BM25, token budgets, and session deduplication — no index, no embeddings.
* 🟢 [ck (seek)](code/ck.md) `0.7.11` — Rust grep-compatible search combining BM25 and local embeddings with RRF, chunk-level blake3 incremental indexing, and MCP cursor pagination.
* 🟡 [Claude Context (Zilliz)](code/claude-context-zilliz.md) `0.1.15` — TypeScript MCP server doing embeddings-only code search backed by Milvus/Zilliz Cloud — the highest-profile entrant, and cloud-gated.
* 🟢 [Code Index MCP](code/code-index-mcp.md) `2.17.1` — Python code indexer with a two-tier shallow/deep index, self-describing escalation errors, and search delegated to the best available native binary.
* 🟢 [codebase-memory-mcp](code/codebase-memory-mcp.md) `0.9.1-rc.1` — Pure-C code knowledge graph with tree-sitter grammars for 158 languages and cross-service HTTP route linking — impressive claims, unusual credibility signals.
* 🟡 [grepai](code/grepai.md) `0.35.0` — Local semantic code search with a clean three-tool call-graph API and Ollama-by-default embeddings.
* 🟢 [Octocode](code/octocode.md) `9.1.1` — MCP server for searching GitHub at large, notable for exposing response density as an explicit per-call tool parameter.
* 🟡 [SeaGOAT](code/seagoat.md) `0.54.17` — The original local-first semantic code search engine, predating the MCP era — historical evidence that integration, not retrieval quality, made this category a market.
* 🟢 [Serena](code/serena.md) `1.6.1` — Python MCP server wrapping Language Server Protocol backends for 40+ languages, offering symbolic navigation and precise refactoring with no index at all.
* 🟢 [Sourcebot](code/sourcebot.md) `5.1.5` — Self-hosted multi-repo code search for teams, with a web UI, auth, and Docker-compose deployment — the enterprise end of the category.

## Documents & RAG

Servers that index documents, PDFs, and project documentation. — [category index](documents/index.md)

* 🟢 [**knowledge-rag**](documents/knowledge-rag.md) `4.8.1` — Python local-first RAG MCP server with a custom inverted-index BM25, ONNX vectors, cross-encoder reranking, and a built-in retrieval-evaluation tool.
* 🟢 [**markdown-vault-mcp**](documents/markdown-vault-mcp.md) `3.1.0` — Python MCP server over generic markdown vaults with adaptive heading chunking, FTS5 BM25 + vector RRF hybrid search, and native OKF awareness.
* 🟢 [Context7](documents/context7.md) `4.0.0` — Cloud service serving up-to-date public library documentation to agents through two tools — the strongest brand in "context for agents", and a positioning foil rather than a rival.
* 🟢 [Docling / docling-mcp](documents/docling.md) `2.118.1` — IBM/LF AI layout-aware document conversion producing a structured DoclingDocument, with reference hierarchical and hybrid chunkers — a dependency comparison for Tarn's format layer.
* 🟡 [kb-mcp-server (txtai)](documents/kb-mcp-server-txtai.md) `0.1.3` — txtai-based knowledge-base server whose distinguishing idea is a portable, shippable index artifact — build once, hand over a single .tar.gz.
* 🟢 [mcp-local-rag (shinpr)](documents/mcp-local-rag-shinpr.md) `0.17.3` — TypeScript local-first RAG server with semantic chunking that preserves code blocks intact, aimed at code and technical documentation.
* 🟢 [mcpdoc (LangChain)](documents/mcpdoc-llmstxt.md) `0.0.10` — Fetch-and-list documentation server that trusts llms.txt as the index — evidence that a large slice of the market needs structure plus fetch, with no ranking at all.
* 🔴 [pdfkb-mcp](documents/pdfkb-mcp.md) `0.7.0` — Abandoned PDF-first RAG server whose pluggable parser-per-tradeoff architecture is the best PDF ingestion design found in this survey.
* 🟢 [zotero-mcp](documents/zotero-mcp.md) `0.9.1` — Zotero research-library MCP server with ~4.6k stars — evidence that a narrow, well-served corpus beats a general one for adoption.

## Obsidian & PKM

Tarn's original category — Obsidian vaults specifically. — [category index](obsidian-pkm/index.md)

* 🟢 [**MCPVault**](obsidian-pkm/mcpvault-bitbonsai.md) `0.14.1` — Zero-dependency TypeScript MCP server over an Obsidian vault's filesystem, with per-query BM25 ranking and minified response fields.
* 🟢 [**Obsidian Copilot**](obsidian-pkm/obsidian-copilot.md) `3.3.3` — In-vault AI assistant plugin with agentic capabilities and a tiered lexical retriever that needs no pre-built index.
* 🟡 [**obsidian-mcp (eddmann)**](obsidian-pkm/obsidian-mcp-eddmann.md) `1.0.0` — Git-backed MCP server for Obsidian vaults with semantic anchors, unified-diff patching, and AWS Lambda deployment.
* 🟡 [**obsidian-mcp (Storks)**](obsidian-pkm/obsidian-mcp-storks.md) `0.1.0` — Python MCP server wrapping the official Obsidian CLI, exposing 54 declaratively-defined tools.
* 🔴 [**obsidian-mcp-assistants**](obsidian-pkm/obsidian-mcp-assistants.md) `0.1.0` — Obsidian plugin that chats with LLM agents over MCP servers, with built-in vault filesystem access.
* 🟢 [**obsidian-mcp-server (cyanheads)**](obsidian-pkm/obsidian-mcp-server-cyanheads.md) `3.2.12` — TypeScript MCP server bridging agents to Obsidian via the Local REST API, with section-addressed reads, surgical patching, and folder-scoped path policy.
* 🟡 [**obsidian-tools**](obsidian-pkm/obsidian-tools-glibalien.md) `unreleased` — Hybrid semantic + keyword MCP server for Obsidian using HyDE, ChromaDB, and cross-encoder reranking.
* 🟢 [**Vault as MCP**](obsidian-pkm/vault-as-mcp-ebullient.md) `0.10.0` — Obsidian plugin running an in-process MCP server over HTTP, with heading-filtered reads and a three-tier glob path ACL.
* 🟡 [**VaultMind**](obsidian-pkm/vaultmind-plugin.md) `3.1.9` — Obsidian plugin turning the vault into an AI workspace via RAG, with LCS-based CJK search and iOS support.
* 🟡 [mcp-obsidian (MarkusPfundstein)](obsidian-pkm/mcp-obsidian-pfundstein.md) `0.2.2` — The most-adopted Obsidian MCP server by a wide margin — a thin REST proxy with no index, keyword-only search, and a stale release cadence.

## Structured data

Databases, schemas, and semantic layers over MCP. — [category index](structured/index.md)

* 🟢 [Cube (semantic layer MCP)](structured/cube-semantic-layer.md) `1.7.17` — Governed measures and dimensions instead of raw DDL — the strongest argument against retrieval, and the counter-position Tarn must be able to answer.
* 🟢 [DataHub / OpenMetadata](structured/datahub-openmetadata.md) `1.7.0` — Two 12k+ star metadata platforms that both rebranded to "context" in 2026 — evidence that the category label Tarn is reaching for is already contested.
* 🟢 [DBHub](structured/dbhub.md) `1.2.0` — Database MCP gateway whose product claim is a measured token budget — 1.4k tokens versus a competitor's 19.0k — delivered via progressive schema disclosure and two default tools.
* 🟢 [dbt MCP](structured/dbt-mcp.md) `2.0.0-beta.1` — Exposes dbt project metadata, model lineage, and the semantic layer — where lineage is the transferable retrieval signal.
* 🟢 [MCP Toolbox for Databases (Google)](structured/mcp-toolbox-databases.md) `1.8.0` — Google's config-as-tools database gateway — a generic runtime where tools are hand-authored YAML, and the category leader by adoption.
* 🟢 [mcp-alchemy](structured/mcp-alchemy.md) `unverified` — SQLAlchemy as a universal database adapter — one abstraction, many backends, which is precisely the shape Tarn's extractor layer needs.
* 🟡 [Postgres MCP Pro](structured/postgres-mcp-pro.md) `0.3.0` — DBA expertise as tools — index tuning, workload analysis, health checks — showing that domain analysis, not data access, is the winning move in a commoditized category.
* 🟢 [Vendor database MCP servers (grouped)](structured/vendor-database-servers.md) `various` — Every database vendor now ships an MCP server — control plane plus SQL, no retrieval — which makes "SQL over MCP" a commodity Tarn should not compete in head-on.
* 🟡 [XiYan-SQL MCP Server](structured/xiyan-sql.md) `0.1.4` — The one structured-data server that actually does retrieval — embedding-based pruning over tables, columns, and values before SQL generation.
