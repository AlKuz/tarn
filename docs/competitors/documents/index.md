# Documents & RAG

Servers that index documents, PDFs, and project documentation.

9 profiles. Cross-cutting analysis is in [analysis.md](../analysis.md); the full roster is in the [bundle index](../index.md).

🟢 active · 🟡 dormant · 🔴 abandoned · ⚫ archived · **bold** = deep dive (source read)

* 🟢 [**knowledge-rag**](knowledge-rag.md) `4.8.1` — Python local-first RAG MCP server with a custom inverted-index BM25, ONNX vectors, cross-encoder reranking, and a built-in retrieval-evaluation tool.
* 🟢 [**markdown-vault-mcp**](markdown-vault-mcp.md) `3.1.0` — Python MCP server over generic markdown vaults with adaptive heading chunking, FTS5 BM25 + vector RRF hybrid search, and native OKF awareness.
* 🟢 [Context7](context7.md) `4.0.0` — Cloud service serving up-to-date public library documentation to agents through two tools — the strongest brand in "context for agents", and a positioning foil rather than a rival.
* 🟢 [Docling / docling-mcp](docling.md) `2.118.1` — IBM/LF AI layout-aware document conversion producing a structured DoclingDocument, with reference hierarchical and hybrid chunkers — a dependency comparison for Tarn's format layer.
* 🟡 [kb-mcp-server (txtai)](kb-mcp-server-txtai.md) `0.1.3` — txtai-based knowledge-base server whose distinguishing idea is a portable, shippable index artifact — build once, hand over a single .tar.gz.
* 🟢 [mcp-local-rag (shinpr)](mcp-local-rag-shinpr.md) `0.17.3` — TypeScript local-first RAG server with semantic chunking that preserves code blocks intact, aimed at code and technical documentation.
* 🟢 [mcpdoc (LangChain)](mcpdoc-llmstxt.md) `0.0.10` — Fetch-and-list documentation server that trusts llms.txt as the index — evidence that a large slice of the market needs structure plus fetch, with no ranking at all.
* 🔴 [pdfkb-mcp](pdfkb-mcp.md) `0.7.0` — Abandoned PDF-first RAG server whose pluggable parser-per-tradeoff architecture is the best PDF ingestion design found in this survey.
* 🟢 [zotero-mcp](zotero-mcp.md) `0.9.1` — Zotero research-library MCP server with ~4.6k stars — evidence that a narrow, well-served corpus beats a general one for adoption.
