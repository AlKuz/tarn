# General-purpose

Mixed local corpora, universal context, and agent memory. The category Tarn is pivoting into.

15 profiles. Cross-cutting analysis is in [analysis.md](../analysis.md); the full roster is in the [bundle index](../index.md).

🟢 active · 🟡 dormant · 🔴 abandoned · ⚫ archived · **bold** = deep dive (source read)

* 🟡 [**engraph**](engraph.md) `1.7.2` — Rust MCP server over Obsidian vaults with five-lane RRF hybrid search, an LLM query orchestrator, 25 tools, and a REST API.
* 🟢 [**lore**](lore.md) `0.1.0` — Rust MCP server and CLI that ingests 90+ document formats from 10 source types into a local Tantivy BM25 index with heading-aware chunking.
* 🟢 [**ostk-recall**](ostk-recall.md) `0.9.3` — Rust multi-corpus MCP retrieval daemon fusing model2vec vectors with Tantivy BM25 over notes, code, and agent session logs behind two tools.
* 🟢 [**Xberg (formerly Kreuzberg)**](xberg.md) `1.1.0` — Rust-core polyglot document intelligence framework extracting 101 formats into a heading-nested DocumentStructure — a candidate dependency for Tarn's pivot, and also an MCP server in its own right.
* 🟢 [Agent-memory tier (mem0, Graphiti/Zep, Letta)](agent-memory-tier.md) `various` — The 100k-star memory category that indexes what the agent said rather than what you wrote — not competitors, but a serious category-confusion risk.
* 🟢 [basic-memory](basic-memory.md) `0.22.1` — AGPL Python knowledge server where the agent writes structured markdown back — typed observations and wikilink relations traversed as a graph, with a paid hosted tier over the same engine.
* 🟢 [cognee](cognee.md) `1.4.2` — Self-hosted knowledge-graph memory for agents exposing exactly three MCP tools while keeping its powerful internals deliberately unexposed.
* 🟢 [Khoj](khoj.md) `2.0.0-beta.28` — AGPL self-hostable "AI second brain" application indexing local docs and the web — the app end of the spectrum, and the shape Tarn should avoid becoming.
* 🟢 [MCP reference servers (filesystem, fetch, memory)](mcp-reference-servers.md) `2026.7.10` — The official baseline every agent already has — filesystem plus the agent's own grep is what Tarn must actually beat, and the honest benchmark target.
* 🟢 [Onyx (formerly Danswer)](onyx.md) `4.5.4` — Enterprise search over 40+ SaaS connectors with permission-aware retrieval — the team/SaaS end of the market, and the best connector abstraction in open source.
* 🟢 [Pharos (PharosRAG)](pharos-rag.md) `unverified` — Local-first agentic RAG over document libraries with page-level citations, and the clearest written rationale for a daemon-plus-thin-client architecture.
* 🟡 [pluck](pluck.md) `0.6.0` — Rust MCP code retrieval whose entire pitch is measured token savings, with every claim gated by a checked-in benchmark file and a --raw escape hatch on every tool.
* 🟡 [Rememex](rememex.md) `2.5.1` — Windows-only Rust local file search over 120+ file types with built-in OCR and EXIF geolocation — proof of demand for exactly Tarn's pivot, left unserved on macOS and Linux.
* 🟢 [SurfSense](surfsense.md) `0.0.36` — Open-source NotebookLM alternative researching the live web through one platform, API, or MCP server — web-first rather than disk-first.
* 🟢 [trusty-search](trusty-search.md) `0.42.3` — Rust machine-wide code search combining BM25, vectors, and a knowledge graph with zero cold-start — one always-on index across every project rather than one per repo.
