---
type: Competitor Profile
title: Docling / docling-mcp
description: IBM/LF AI layout-aware document conversion producing a structured DoclingDocument, with reference hierarchical and hybrid chunkers — a dependency comparison for Tarn's format layer.
resource: https://github.com/docling-project/docling
tags: [documents, python, layout-aware, pdf, chunking, dependency]
timestamp: 2026-08-09
category: documents
version: 2.118.1
version_checked: 2026-08-09
status: active
depth: profile
---

# Docling + docling-mcp (IBM / LF AI & Data)

| Project | Version | Stars | Activity | License |
|---|---|---|---|---|
| [docling](https://github.com/docling-project/docling) | **v2.118.1** (2026-08-07) | ~64,430 | 2026-08-08 | MIT |
| [docling-mcp](https://github.com/docling-project/docling-mcp) | **v3.0.0** (2026-07-31) | ~705 | 2026-07-31 | MIT |

## What it does

Converts PDF, DOCX, PPTX, XLSX, HTML, and images into a **`DoclingDocument`** — structured JSON
with **true layout awareness**: reading order, tables, and figures reconstructed rather than
guessed from text flow. Its **Hierarchical** and **Hybrid** chunkers are the reference
implementations of structure-aware chunking in the Python ecosystem.

`docling-mcp` wraps this as an MCP server for conversion and document generation, with optional
Milvus upload/retrieval and Remote / Local / Hybrid deployment modes.

## Relevance to Tarn — a dependency comparison, not a rivalry

docling-mcp is a **conversion and generation** server. It is weak on exactly what Tarn is: a
persistent, ranked, incrementally-updated retrieval index over a corpus. So the natural
relationship is layered — Docling supplies structure, Tarn supplies retrieval.

**The important comparison is against [xberg](../general/xberg.md)**, which occupies the same
slot in Tarn's architecture:

| | Docling | Xberg |
|---|---|---|
| Language | Python | **Rust** |
| Integration for Tarn | Sidecar process or `docling-serve` HTTP | **Native crate dependency** |
| Formats | PDF, Office, HTML, images | 101 formats / 115 extensions |
| Structure output | `DoclingDocument` | `DocumentStructure` (heading-nested) |
| Maturity | 64k★, LF AI governance | 8.9k★ |
| License | MIT | MIT |

Docling is far more established and has stronger institutional backing. Xberg is Rust, so it
links directly into Tarn with no process boundary, no Python runtime, and no serialization
round-trip. **For a single-binary Rust tool that difference is decisive**, which is why
[lore](../general/lore.md) chose Xberg (as `kreuzberg`) and why the recommendation in
[xberg.md](../general/xberg.md) stands.

Docling remains the fallback worth naming: if Xberg's format coverage or maintenance falters,
a `docling-serve` sidecar is the credible plan B, at the cost of the single-binary property.

**What to take regardless:** the hierarchical chunker's model maps almost 1:1 onto Tarn's
section model, and layout-aware reading-order reconstruction is the specific capability that
separates a real PDF pipeline from a text dump.
