---
type: Competitor Profile
title: cognee
description: Self-hosted knowledge-graph memory for agents exposing exactly three MCP tools while keeping its powerful internals deliberately unexposed.
resource: https://github.com/topoteretes/cognee
tags: [general, python, knowledge-graph, memory, minimal-surface]
timestamp: 2026-08-09
category: general
version: 1.4.2
version_checked: 2026-08-09
status: active
depth: profile
---

# cognee (topoteretes)

| Field | Value |
|---|---|
| Repository | <https://github.com/topoteretes/cognee> |
| Version | **v1.4.2** (2026-08-08) |
| Stars / activity | ~29,884 · last push 2026-08-08 |
| Language / license | Python · Apache-2.0 |
| MCP tools | **3** — `remember`, `recall`, `forget` |

## What it does

Self-hosted knowledge-graph memory that persists across agent sessions. `remember` with a
`session_id` writes to a fast session cache; without one, to permanent graph memory. `recall`
auto-routes — session cache first, falling through to the graph.

Self-hostable, but the graph pipeline is **LLM-dependent**: entity extraction needs a model.

## The lesson: what is *not* exposed

`cognify`, `search`, `list_data`, `delete`, and `prune` all exist and are **deliberately kept
internal**. A 30k-star project chose to expose three verbs and hide the machinery.

Set against the rest of this survey:

| Project | Tools | Stars |
|---|---|---|
| [ostk-recall](ostk-recall.md) | 2 | ~6 |
| [DBHub](../structured/dbhub.md) | 2 | ~3,309 |
| [Context7](../documents/context7.md) | 2 | ~60,466 |
| **cognee** | **3** | **~29,884** |
| [lore](lore.md) | 6 | ~17 |
| [codesearch](../code/codesearch.md) | 6 | ~64 |
| [engraph](engraph.md) | 25 (+26 REST) | ~164 |
| [markdown-vault-mcp](../documents/markdown-vault-mcp.md) | 33 | ~27 |

**Tool count does not predict adoption, and the largest surfaces belong to the least-adopted
projects.** Small surfaces are at worst harmless and at best a genuine quality advantage in
tool-selection accuracy. Tarn should treat every added tool as needing justification.

## Vocabulary convergence

cognee and [ostk-recall](ostk-recall.md) landed on `remember` / `recall` independently. Those
verbs are becoming the category convention for memory-shaped servers — which is a reason for
Tarn to *avoid* them. Tarn is retrieval over artifacts you already have, not memory the agent
accumulates, and adopting memory vocabulary invites being benchmarked against
[mem0 and friends](agent-memory-tier.md) on the wrong axis.

## Relevance to Tarn

Low technically — graph memory over agent-extracted entities is a different product. High as
surface-design evidence.
