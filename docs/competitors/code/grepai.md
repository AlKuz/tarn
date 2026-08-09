---
type: Competitor Profile
title: grepai
description: Local semantic code search with a clean three-tool call-graph API and Ollama-by-default embeddings.
resource: https://github.com/yoanbernabeu/grepai
tags: [code, semantic-search, call-graph, ollama, agent-skills]
timestamp: 2026-08-09
category: code
version: 0.35.0
version_checked: 2026-08-09
status: dormant
depth: profile
---

# grepai (yoanbernabeu)

| Field | Value |
|---|---|
| Repository | <https://github.com/yoanbernabeu/grepai> |
| Version | **v0.35.0** (2026-03-16) |
| Stars / activity | ~1,809 · last push **2026-06-22** — ~7 weeks quiet at review |
| Language / license | GitHub reports C · MIT |
| MCP tools | `grepai_search`, `grepai_trace_callers`, `grepai_trace_callees`, `grepai_trace_graph` |

## What it does

Semantic vector search plus call graphs, 100% local — but it **requires an embedding
provider**: Ollama by default, or LM Studio or OpenAI. Ships a file watcher that keeps the
index fresh automatically, and publishes **Agent Skills** wrapping its MCP tools.

## Ideas worth taking

- **The call-graph triad.** `trace_callers` / `trace_callees` / `trace_graph` is a minimal,
  well-named API for graph traversal — three verbs covering inbound, outbound, and full
  expansion. If Tarn ever exposes its wikilink graph (it already parses links into the index),
  this naming is a good template: `trace_backlinks` / `trace_links` / `trace_graph`.
- **Ollama as the default embedding backend.** This is how you offer semantic retrieval while
  staying local-first *without* bundling a model in your binary — the user brings their own
  inference. It is a useful fallback path if Tarn ever wants a hybrid lane without taking
  [engraph](../general/engraph.md)'s mandatory ~300 MB GGUF download.
- **Publishing Agent Skills alongside MCP tools** as a second distribution surface.

## Relevance to Tarn

Moderate. Code-only, embedding-dependent, and now quiet. The graph API naming and the
bring-your-own-Ollama pattern are the transferable parts.
