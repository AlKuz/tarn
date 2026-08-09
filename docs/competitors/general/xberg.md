---
type: Competitor Profile
title: Xberg (formerly Kreuzberg)
description: Rust-core polyglot document intelligence framework extracting 101 formats into a heading-nested DocumentStructure — a candidate dependency for Tarn's pivot, and also an MCP server in its own right.
resource: https://github.com/xberg-io/xberg
tags: [general, rust, extraction, dependency, document-structure, ocr, chunking, mcp]
timestamp: 2026-08-09
category: general
version: 1.1.0
version_checked: 2026-08-09
status: active
depth: deep-dive
---

# Xberg (formerly Kreuzberg) — dependency evaluation

> **This is a build-vs-buy decision, not a rival to beat — but it is partly both.** Xberg is
> the Rust document-extraction core that [lore](lore.md) depends on (as `kreuzberg`), and it
> produces exactly the primitive Tarn's pivot needs: a **`DocumentStructure` with
> heading-driven section nesting**, derived uniformly across 101 formats. It also ships its
> own MCP server, which makes it a partial competitor at the edges.

## Overview

| Field | Value |
|---|---|
| **Repository** | <https://github.com/xberg-io/xberg> |
| **Former name** | **Kreuzberg** — [lore](lore.md) still depends on it as `kreuzberg = "4.9"` |
| **Language** | **Rust core** (edition 2024, rust-version 1.91) |
| **Version** | **1.1.0** |
| **License** | MIT |
| **Stars** | ~8,932 |
| **Last activity** | 2026-08-08 |
| **Status** | Active |
| **Crate** | `xberg` on crates.io |
| **Bindings** | Python, Node, WASM, Java, Go, C#, PHP, Ruby, Elixir, Dart, Swift, Zig, Kotlin, C FFI |
| **Surfaces** | Library, CLI (12 commands), REST API (`xberg serve`), **MCP server** (`xberg mcp`), Docker, Helm |

### Maturity Assessment

Substantially more resourced than any competitor in this bundle: `SECURITY.md`,
`THIRD_PARTY_LICENSES.md`, `ATTRIBUTIONS.md`, `PLATFORM_SUPPORT.md`, a Taskfile, Helm charts,
an artifacthub descriptor, a docs site, and package manifests for a dozen ecosystems in the
repo root. ~8.9k stars.

The rename from Kreuzberg to Xberg is worth tracking: `lore` pins `kreuzberg = "4.9"` while
the project now ships `xberg` 1.1.0, so the crate identity and version line both changed.

---

## Why it matters to Tarn

### The capability set

| Capability | Detail |
|---|---|
| **101 document formats** | 115 file extensions — PDF, Office, images, HTML, email, e-books, scientific publications, structured data. MIME detection, streaming for multi-GB files. |
| **Layout & tables** | ML layout models (PP-DocLayout-V3, RT-DETR) and table structure (TATR, SLANet) reconstruct **reading order** and cell grids into clean Markdown. |
| **OCR on demand** | Tesseract, PaddleOCR, Candle, or VLM backends with **fallback chains**, confidence scores, and language auto-detection. |
| **Archives, traversed** | Recursive `.zip`/`.tar`/`.gz`/`.7z` extraction — documents inside documents — guarded by **zip-bomb, compression-ratio, and nesting-depth limits**. |
| **Code intelligence** | Functions, classes, imports, symbols, docstrings from **371 languages**; syntax-aware chunking. |
| **URLs & crawling** | Fetch a URL or crawl and follow links (`url-ingestion` feature). |
| **Audio/video transcription** | Whisper ONNX (`transcription` feature). |
| **Embeddings & reranking** | Local ONNX or hosted; sparse and late-interaction; cross-encoder rerank. |
| **Enrichment** | NER, YAKE/RAKE keywords, summarization, translation, redaction, language detection, token reduction. |
| **6 output formats** | Text, Markdown, Djot, HTML, JSON tree, Structured (JSON with OCR metadata and bounding boxes). |

### `DocumentStructure` — the primitive that matters

`crates/xberg/src/types/document_structure.rs` and `builder.rs`:

```rust
pub struct DocumentStructure {
    /// All nodes in document/reading order.
    pub nodes: Vec<DocumentNode>,
    …
}

pub struct DocumentStructureBuilder {
    doc: DocumentStructure,
    section_stack: Vec<(u8, NodeIndex)>,   // (heading level, node)
    /// Stack of active container nodes (Quote, Admonition, Slide, etc.).
    container_stack: Vec<NodeIndex>,
    node_count: u32,
}
```

A `section_stack` of `(heading_level, node)` pairs **is** a heading path, maintained during
construction. The bindings documentation describes the output as a *"hierarchical
`DocumentStructure` containing heading-driven section nesting"*, and the layout models
reconstruct reading order for PDFs so the heading hierarchy is recoverable even from a
scanned page.

**This is the answer to the central gap identified across this whole survey.** Every document
competitor degrades to fixed windows on non-markdown: [knowledge-rag](../documents/knowledge-rag.md)
uses 1000/200-character windows for PDF/docx/xlsx/pptx;
[markdown-vault-mcp](../documents/markdown-vault-mcp.md) treats non-markdown as "attachments".
The reason is that recovering structure from a PDF is genuinely hard. Xberg has already done
that work, in Rust, under MIT.

Supporting modules in `crates/xberg/src/chunking/` reinforce it: `headings.rs`,
`boundaries.rs`, `boundary_detection.rs`, `page_spans.rs`, `yaml_section.rs`, `semantic/`,
`rag.rs`, `tokenizer_cache.rs`, `validation.rs`.

**`page_spans.rs` deserves particular note** — page spans are to a PDF what `heading_path` is
to markdown: the natural citation and addressing unit. A general-purpose Tarn needs a
per-format notion of "where did this come from", and Xberg already computes both.

---

## The dual role — and the risk

Xberg is not purely infrastructure. It ships **`xberg mcp`** (stdio or HTTP), a REST API, and
a CLI with `embed` and `chunk` commands. Its MCP server surface includes cache management
(`cache_stats`, `cache_clear`, `cache_manifest`, `cache_warm`), embeddings and NER model
listing, and an `allowed_hosts` policy module.

So the honest framing is:

- **As a dependency** it solves Tarn's hardest pivot problem — multi-format extraction with
  structure preserved — better than Tarn could in any reasonable timeframe.
- **As a product** it overlaps at the edges: extract-and-serve over MCP. What it does *not*
  do is maintain a persistent, ranked, incrementally-updated retrieval index over a corpus.
  Its MCP server is document-oriented (point it at a thing, get content back), not
  corpus-oriented (ask a question, get ranked sections from across everything).

That distinction is Tarn's product line, and it is a real one — but it is thinner than
"Xberg is just a library" would suggest, and it should be stated plainly rather than assumed.

---

## Build-vs-buy assessment

### For taking the dependency

1. **It is the single highest-leverage decision in the pivot.** 101 formats, OCR fallback
   chains, layout models, table reconstruction, and archive traversal represent years of work
   that is not Tarn's differentiator.
2. **Rust, MIT, edition 2024** — no FFI boundary, no Python sidecar, no license friction.
3. **`DocumentStructure` maps onto `heading_path` directly.** Tarn would consume the section
   nesting rather than re-deriving it, and would extend its existing model to formats it
   cannot currently read at all.
4. **Cargo feature flags** allow a lean default build — the pattern [lore](lore.md) already
   demonstrates in production (`ocr`, `iwork`, `tree-sitter` each resolve to a
   `kreuzberg/<feature>`).
5. **Security work already done** on the nasty parts: zip-bomb, compression-ratio, and
   nesting-depth guards on recursive archives; an `allowed_hosts` policy for URL ingestion.
6. **Precedent**: lore is shipping on it.

### Against, or to weigh carefully

1. **Scope is enormous.** Transcription, NER, translation, embeddings, reranking, crawling,
   structured LLM extraction. Tarn needs maybe 10% of this, and the dependency's decisions
   would shape Tarn's build matrix. Feature-flag discipline is mandatory, not optional.
2. **Version and identity churn.** Kreuzberg → Xberg with a crate rename and a reset to 1.1.0.
   A dependency that renamed once may again.
3. **Heavy optional dependencies** — ONNX Runtime for layout/OCR/embeddings pulls in the same
   `ort` version-pinning pain [codanna](../code/codanna.md) documents in its `fastembed` pin.
4. **Chunking-decision transfer.** Delegating extraction is clean; delegating *chunking*
   means chunk quality becomes the dependency's decision. Tarn should take `DocumentStructure`
   and do its own sectioning on top, rather than consuming `chunking/` wholesale — that keeps
   `heading_path` semantics under Tarn's control.
5. **Partial competitive overlap** via `xberg mcp`.
6. **Binary size and build time** even with features trimmed.

### Recommendation

**Take the dependency for extraction; keep sectioning and indexing in Tarn.**

Specifically: consume `DocumentStructure` (and `page_spans` for paginated formats), derive
Tarn's own `heading_path`-carrying sections from the node hierarchy, and index those. Do not
consume `chunking/` — chunk boundaries are Tarn's product surface and its
`Buildable`/`Configurable` pattern already models pluggable strategies. Gate every optional
capability behind a Cargo feature, defaulting to the smallest set that reads PDF, Office, and
plain text without an ONNX runtime.

That preserves the thing that makes Tarn distinct — sections as the ranked retrieval unit,
with a full heading path — while acquiring the format coverage that the pivot otherwise
cannot reach.

---

## Comparison with Tarn

| Dimension | Xberg | Tarn |
|---|---|---|
| Role | **Extraction / document intelligence** | **Retrieval over a corpus** |
| Formats | **101 (115 extensions)** | Markdown |
| Structure output | **`DocumentStructure`, heading-nested** | `heading_path` sections |
| Page spans | **Yes** | N/A |
| OCR / layout / tables | **Yes** (ML models, fallback chains) | No |
| Persistent index | **No** | **Yes** |
| Ranked corpus retrieval | **No** | **Yes** (BM25) |
| Incremental update | Content-hash caching | Observer-driven index sync |
| MCP surface | Document-oriented (`xberg mcp`) | Corpus-oriented |
| Concurrency control | N/A | `RevisionToken` |
| Obsidian syntax | No | Full parser |
| License | MIT | — |

### What Tarn should take even without adopting the dependency

- **`DocumentStructure` as a design target.** Whatever produces it, the pivot needs one
  internal representation — nodes in reading order plus a section stack — that every format
  extractor populates. Then `heading_path` derivation is written once, not per format.
- **Page spans as the addressing unit for paginated formats.** "Section 2.1" for markdown,
  "page 14" for a PDF, "symbol path" for code — one abstraction, per-format instantiations.
- **A container stack alongside the section stack.** Xberg's builder tracks Quote /
  Admonition / Slide containers separately so body nodes parent correctly. Obsidian callouts
  are exactly this shape and Tarn will need the distinction.
- **Guard recursive extraction explicitly** — zip-bomb, compression-ratio, and nesting-depth
  limits are not optional once archives are in scope.
- **Extraction fallback chains** with confidence scores, rather than a single parser per
  format. This is the same idea [pdfkb-mcp](../documents/pdfkb-mcp.md) implements as pluggable
  per-tradeoff parsers.
