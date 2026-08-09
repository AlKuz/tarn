---
type: Competitor Profile
title: pdfkb-mcp
description: Abandoned PDF-first RAG server whose pluggable parser-per-tradeoff architecture is the best PDF ingestion design found in this survey.
resource: https://github.com/juanqui/pdfkb-mcp
tags: [documents, python, pdf, pluggable-parsers, abandoned]
timestamp: 2026-08-09
category: documents
version: 0.7.0
version_checked: 2026-08-09
status: abandoned
depth: profile
---

# pdfkb-mcp (juanqui)

| Field | Value |
|---|---|
| Repository | <https://github.com/juanqui/pdfkb-mcp> |
| Version | **v0.7.0** (2025-09-13) |
| Stars / activity | ~13 · last push **2025-09-15** — ~11 months stale |
| Language / license | Python · MIT |
| Status | **Abandoned** |

## Why an abandoned 13-star project earns a profile

Because its architecture answers a question Tarn's pivot must answer, and because being dead
means the ideas can be borrowed freely.

**Five pluggable PDF parsers, selected by tradeoff:**

| Parser | Chosen when |
|---|---|
| PyMuPDF4LLM | fast |
| Marker | balanced |
| Docling | tables matter |
| MinerU | academic papers |
| LLM-based | complex layouts nothing else handles |

**Four chunking strategies:** LangChain, semantic, page-based, unstructured.

Plus BM25 hybrid retrieval, a Qwen3 reranker, and an optional web UI.

## The transferable idea

**There is no single correct PDF parser.** Speed, table fidelity, equation handling, and
scanned-page OCR are genuinely conflicting objectives, and a general-purpose corpus contains
all four kinds of document. Hard-coding one parser means being wrong for most of the corpus.

Two ways this lands in Tarn's design:

1. **The extractor should be a trait with per-format *and per-tradeoff* implementations**,
   selected by config — which fits Tarn's existing `Buildable`/`Configurable` dispatch-enum
   pattern exactly. A `PdfExtractorConfig` enum with `Fast` / `Tables` / `Ocr` variants is the
   natural shape.
2. **Selection belongs in per-source config**, not global config — the same
   `[[sources]]`-with-overrides model [ostk-recall](../general/ostk-recall.md) and
   [lore](../general/lore.md) both use. A source pointed at scanned invoices and a source
   pointed at generated API docs want different parsers.

[xberg](../general/xberg.md) already implements the analogous idea for OCR (Tesseract /
PaddleOCR / Candle / VLM with **fallback chains and confidence scores**), which is the same
principle with automatic degradation rather than manual selection.

## Relevance to Tarn

Dead as a competitor; its ingestion architecture is the best PDF design in the bundle.
