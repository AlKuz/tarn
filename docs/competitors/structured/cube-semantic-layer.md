---
type: Competitor Profile
title: Cube (semantic layer MCP)
description: Governed measures and dimensions instead of raw DDL — the strongest argument against retrieval, and the counter-position Tarn must be able to answer.
resource: https://github.com/cube-js/cube
tags: [structured, rust, semantic-layer, curation-vs-retrieval, counter-argument]
timestamp: 2026-08-09
category: structured
version: 1.7.17
version_checked: 2026-08-09
status: active
depth: profile
---

# Cube — semantic layer MCP

| Field | Value |
|---|---|
| Repository | <https://github.com/cube-js/cube> · [MCP docs](https://docs.cube.dev/docs/integrations/mcp-server) |
| Version | **v1.7.17** (2026-08-07) |
| Stars / activity | ~20,576 · last push 2026-08-09 |
| Language / license | **Rust** (Cube Core) · Apache-2.0 core, commercial tier on top |
| Local-first | ❌ hosted MCP endpoint per tenant, OAuth over HTTPS |

## What it does

Exposes **governed measures and dimensions with descriptions** rather than raw DDL. The agent
introspects a curated catalog, sends a structured request (measures, dimensions, filters, time
range), and Cube compiles it to SQL with access rules applied.

## Why it is in this bundle: the counter-argument

**Cube is the strongest case against retrieval, and Tarn should be able to answer it.**

The schema-context problem has two possible solutions:

| | Approach | Cost | Scales with |
|---|---|---|---|
| **Retrieval** ([XiYan-SQL](xiyan-sql.md), Tarn) | Index everything, rank by relevance | Index maintenance, ranking quality | Corpus size, automatically |
| **Curation** (Cube) | Pre-author a small meaningful surface | Human authoring effort | Human effort, linearly |

Curation wins where a small, stable, high-value surface exists and someone is paid to maintain
it — enterprise analytics being the canonical case. It produces better answers than ranking
because a human already did the ranking, permanently.

**Tarn's answer must be specific, not dismissive:** curation does not scale to a personal or
mixed corpus, and it cannot be done in advance. Nobody is going to hand-author a semantic layer
over their notes, their source tree, and a folder of PDFs — the corpus is large, heterogeneous,
changes daily, and has no owner whose job it is to describe it. Retrieval is the only viable
answer when the corpus is unowned and unbounded.

The same argument appears in the documents category:
[mcpdoc](../documents/mcpdoc-llmstxt.md) has ~1,024 stars serving `llms.txt`-indexed docs with
**no ranking at all**, because for a well-curated documentation set the author already did the
work. Where good curation exists, Tarn adds little; where it does not — which is most personal
and project corpora — Tarn is the only option.

Knowing which side of that line a given corpus falls on is worth stating explicitly in Tarn's
own documentation. It is more credible than claiming ranking is always better.
