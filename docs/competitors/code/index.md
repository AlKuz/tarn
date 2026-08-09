# Code context

Servers that index and serve source code to agents.

12 profiles. Cross-cutting analysis is in [analysis.md](../analysis.md); the full roster is in the [bundle index](../index.md).

🟢 active · 🟡 dormant · 🔴 abandoned · ⚫ archived · **bold** = deep dive (source read)

* 🟢 [**Codanna**](codanna.md) `0.13.2` — Rust MCP server indexing code symbols and call graphs across 15 languages via tree-sitter, plus a separate heading-aware document RAG collection.
* 🟢 [**codesearch (flupkede)**](codesearch.md) `1.2.4` — Rust MCP server with AST-aware chunking for 17 languages, arroy + Tantivy hybrid RRF retrieval, and cross-repository federated search.
* 🟢 [**Probe**](probe.md) `0.6.0-rc330` — Stateless Rust code search with AST-aware extraction, a boolean query language, SIMD-accelerated BM25, token budgets, and session deduplication — no index, no embeddings.
* 🟢 [ck (seek)](ck.md) `0.7.11` — Rust grep-compatible search combining BM25 and local embeddings with RRF, chunk-level blake3 incremental indexing, and MCP cursor pagination.
* 🟡 [Claude Context (Zilliz)](claude-context-zilliz.md) `0.1.15` — TypeScript MCP server doing embeddings-only code search backed by Milvus/Zilliz Cloud — the highest-profile entrant, and cloud-gated.
* 🟢 [Code Index MCP](code-index-mcp.md) `2.17.1` — Python code indexer with a two-tier shallow/deep index, self-describing escalation errors, and search delegated to the best available native binary.
* 🟢 [codebase-memory-mcp](codebase-memory-mcp.md) `0.9.1-rc.1` — Pure-C code knowledge graph with tree-sitter grammars for 158 languages and cross-service HTTP route linking — impressive claims, unusual credibility signals.
* 🟡 [grepai](grepai.md) `0.35.0` — Local semantic code search with a clean three-tool call-graph API and Ollama-by-default embeddings.
* 🟢 [Octocode](octocode.md) `9.1.1` — MCP server for searching GitHub at large, notable for exposing response density as an explicit per-call tool parameter.
* 🟡 [SeaGOAT](seagoat.md) `0.54.17` — The original local-first semantic code search engine, predating the MCP era — historical evidence that integration, not retrieval quality, made this category a market.
* 🟢 [Serena](serena.md) `1.6.1` — Python MCP server wrapping Language Server Protocol backends for 40+ languages, offering symbolic navigation and precise refactoring with no index at all.
* 🟢 [Sourcebot](sourcebot.md) `5.1.5` — Self-hosted multi-repo code search for teams, with a web UI, auth, and Docker-compose deployment — the enterprise end of the category.
