---
type: Competitor Profile
title: Pharos (PharosRAG)
description: Local-first agentic RAG over document libraries with page-level citations, and the clearest written rationale for a daemon-plus-thin-client architecture.
resource: https://github.com/Laurent00TT/PharosRAG
tags: [general, python, daemon, qdrant, citations, pdf, tool-contract]
timestamp: 2026-08-09
category: general
version: unverified
version_checked: 2026-08-09
status: active
depth: profile
---

# Pharos / PharosRAG (Laurent00TT)

| Field | Value |
|---|---|
| Repository | <https://github.com/Laurent00TT/PharosRAG> |
| Version | **no GitHub releases — version unverified** |
| Stars / activity | ~286 · last push 2026-07-22 |
| Language / license | Python 3.10+ · MIT |
| Store | Embedded **Qdrant** + a resident daemon |
| Note | The 12-part learning-series documentation is in Chinese; code, CLI, and commits are in English |

## What it does

Indexes PDFs, **scans**, docx, pptx, and xlsx into a local knowledge base with **citations
traceable back to the page**. Hybrid retrieval with a grounding step; in MCP mode the agent
decides when to retrieve, how to rewrite queries, and whether to multi-hop.

## Two architecture decisions worth taking

**1. The daemon rationale, stated explicitly.**

Embedded Qdrant takes an exclusive lock, and models are slow to load. If every MCP client spawns
its own stdio process, they fight over the index and each pays the model load cost. Pharos
inverts it: **a resident daemon owns the resources, and MCP becomes a thin, fast-starting
adapter.**

[ostk-recall](ostk-recall.md) reached the identical conclusion independently, with a socket and
a singleton lock. **Two of the closest architectural analogues to a general-purpose Tarn both
ended up here**, which makes it a default rather than a preference — and it is expensive to
retrofit once an HTTP transport exists.

**2. One tool contract shared by both surfaces.**

Pharos exposes an HTTP API (`pharos serve`, closed-pipeline QA with citations) *and* MCP
(`pharos mcp`, agentic retrieval), both enforced by a single `toolcore` contract so the two
cannot drift. Any project that grows a second transport needs this; without it the surfaces
diverge and the docs stop matching one of them.

## And one retrieval idea

**Page-level citations.** For a paginated document, the page is the natural addressing and
citation unit — exactly what `heading_path` is for markdown. Tarn's general-purpose pivot needs
a per-format notion of provenance, and "section for markdown, page for PDF, symbol for code" is
the right generalization. [xberg](xberg.md)'s `page_spans` module computes precisely this.

## Relevance to Tarn

Moderate as a competitor (Python, document-library focus, modest adoption); high on two
specific architecture decisions Tarn should make before adding transports.
