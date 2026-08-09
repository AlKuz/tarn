---
type: Competitor Profile
title: dbt MCP
description: Exposes dbt project metadata, model lineage, and the semantic layer — where lineage is the transferable retrieval signal.
resource: https://github.com/dbt-labs/dbt-mcp
tags: [structured, python, dbt, lineage, graph-signal]
timestamp: 2026-08-09
category: structured
version: 2.0.0-beta.1
version_checked: 2026-08-09
status: active
depth: profile
---

# dbt MCP (dbt Labs)

| Field | Value |
|---|---|
| Repository | <https://github.com/dbt-labs/dbt-mcp> |
| Version | **v2.0.0-beta.1** (2026-07-30) |
| Stars / activity | ~595 · last push 2026-08-05 |
| Language / license | Python · Apache-2.0 |
| Local-first | Hybrid — local dbt Core commands, but the Semantic Layer and Discovery API need dbt Cloud tokens |

## What it does

Exposes dbt project metadata, **model lineage**, the dbt Semantic Layer, and dbt command
execution to agents.

## The transferable idea: lineage as a retrieval signal

dbt's graph is a first-class artifact — every model declares its upstream dependencies, so
"what feeds this table" and "what breaks if I change it" are answerable structurally rather than
by search.

Tarn's equivalents already exist in its index and are not exposed:

- **wikilinks** between notes (parsed today, stored in `SectionEntry`),
- **imports** between source files, once code is a source kind,
- **`![[embeds]]`** as a containment relation.

[engraph](../general/engraph.md) demonstrates the payoff by making wikilink expansion a
**retrieval lane** — its headline example surfaces a note that never mentions the query term,
reached because it is linked from one that does. [grepai](../code/grepai.md) exposes the same
idea as a clean three-verb API (`trace_callers` / `trace_callees` / `trace_graph`).

**Tarn already stores links in the index; exposing graph queries is genuinely low-hanging
fruit** — it needs no new extraction, no new storage, and no model. It is the cheapest ranking
lane available after BM25F field weighting.

## Relevance to Tarn

Low as a competitor (vertical, partly cloud-gated). The lineage-as-signal framing is the reason
to keep the profile.
