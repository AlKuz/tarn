---
type: Competitor Profile
title: pluck
description: Rust MCP code retrieval whose entire pitch is measured token savings, with every claim gated by a checked-in benchmark file and a --raw escape hatch on every tool.
resource: https://github.com/hunhee98/pluck
tags: [general, code, rust, bm25f, benchmarks, token-savings]
timestamp: 2026-08-09
category: general
version: 0.6.0
version_checked: 2026-08-09
status: dormant
depth: profile
---

# pluck (hunhee98)

| Field | Value |
|---|---|
| Repository | <https://github.com/hunhee98/pluck> |
| Version | **v0.6.0** (2026-06-02); crates.io `pluck-mcp` 0.6.0 |
| Stars / activity | ~40 · last push **2026-06-02** — ~2 months quiet |
| Language / license | Rust · MIT |
| MCP tools | ~7 — `plan`, `search`, `peek`, `symbol`, `impact`, `deps`, `digest` |

## What it does

AST-chunked code retrieval (and CI log retrieval) with **BM25F** — field-weighted BM25 — plus a
semantic lane, session-aware.

## Three ideas worth taking

**1. BM25F is the cheapest ranking upgrade available to Tarn.**

Field-weighted BM25 scores title, heading, path, and body with different weights rather than
treating a document as one bag of words. Tarn already stores `heading_path` and path
separately, so weighting them is a change to the scorer, not to the index.

Three independent projects converge here: pluck uses BM25F; [lore](lore.md) hard-codes boosts
(title 3.0 / **section 2.0** / body 1.0); [markdown-vault-mcp](../documents/markdown-vault-mcp.md)
exposes per-column weights over `path, title, folder, heading, content, summary` as user
config. The configurable form is the better target.

**2. Every claim gated by a checked-in benchmark.**

`benchmarks/baseline.json` backs the headline numbers: *"84–88% fewer tokens on code reads,
71% shorter CI logs, 0.07ms warm search."* Alongside [DBHub](../structured/dbhub.md)'s token
table and [knowledge-rag](../documents/knowledge-rag.md)'s `evaluate_retrieval` tool, this is
the third instance of the same lesson: **quantify the context savings or nobody believes you.**
Tarn's core claim is measurable and currently unmeasured.

**3. A `--raw` fallback on every tool.**

Each tool can return unprocessed output. The agent never loses a capability by using pluck
instead of the underlying primitive, which removes the main reason to distrust a
context-reducing layer. Tarn's analogue: any `compact`/snippet mode must have a documented way
to get the full section, or agents will route around it.

Also notable: `pluck.plan` returns *"3–5 next-call recommendations"* rather than content — a
tool whose entire output is guidance, which is the strongest form of the pattern
[codanna](../code/codanna.md) implements as templated hints.

## Relevance to Tarn

Code-only and now quiet, so low as a competitor. Its positioning discipline — benchmark-gated
claims, an explicit escape hatch — is the transferable part.
