---
type: Competitor Profile
title: DBHub
description: Database MCP gateway whose product claim is a measured token budget — 1.4k tokens versus a competitor's 19.0k — delivered via progressive schema disclosure and two default tools.
resource: https://github.com/bytebase/dbhub
tags: [structured, typescript, progressive-disclosure, token-budget, benchmarks]
timestamp: 2026-08-09
category: structured
version: 1.2.0
version_checked: 2026-08-09
status: active
depth: profile
---

# DBHub (bytebase)

| Field | Value |
|---|---|
| Repository | <https://github.com/bytebase/dbhub> |
| Version | **v1.2.0** (2026-07-31) |
| Stars / activity | ~3,309 · last push 2026-08-08 |
| Language / license | TypeScript · MIT |
| MCP tools | **2 by default** — `execute_sql`, `search_objects` — plus 2 opt-in (`explain_sql`, `health_check`) and user-defined custom tools in `dbhub.toml` |
| Databases | Postgres, MySQL, SQL Server, MariaDB, SQLite |

## Why this is the most relevant profile in the structured category

**It validates Tarn's core thesis in a different domain, and it does so with numbers.**

The problem DBHub solves is that a database schema is too large for a context window — the same
shape as "a note is too large for a context window". Its answer is **progressive disclosure**:
`search_objects` reveals the schema in layers as the agent drills down, rather than dumping it.
That is section-level retrieval applied to schemas.

And its marketing wedge is a **measured token comparison**, published in the README: its default
configuration costs **~1.4k tokens versus MCP Toolbox's ~19.0k across 28 tools — "13–14× fewer."**

Two things to take:

1. **Publish a measured token-cost comparison.** This is the third independent instance of the
   pattern in this survey ([pluck](../general/pluck.md)'s benchmark-gated claims,
   [knowledge-rag](../documents/knowledge-rag.md)'s `evaluate_retrieval` tool). Tarn's claim —
   section retrieval costs fewer tokens per correct answer than whole-file retrieval — is
   exactly this kind of claim, and it is currently unmeasured. The comparison should be against
   [filesystem MCP + grep](../general/mcp-reference-servers.md).
2. **Custom tools defined in config.** `dbhub.toml` lets a user declare their own named tools.
   Generalized to Tarn, that is a user declaring "my SQL corpus" or "my PDF archive" as a named,
   directly-callable tool — a way to give a general-purpose server the *feel* of a vertical one,
   which is the adoption problem [zotero-mcp](../documents/zotero-mcp.md) exposes.

Note also that DBHub carries **two default tools** and ~3.3k stars, while the 28-tool
alternative it benchmarks against is the one it beats on cost.

## What it does not do

No retrieval, no ranking, no index. `search_objects` filters and reveals; it does not score.
Every structured-data server in this category shares that property — see
[XiYan-SQL](xiyan-sql.md) for the one exception.
