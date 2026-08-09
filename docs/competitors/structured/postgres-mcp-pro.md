---
type: Competitor Profile
title: Postgres MCP Pro
description: DBA expertise as tools — index tuning, workload analysis, health checks — showing that domain analysis, not data access, is the winning move in a commoditized category.
resource: https://github.com/crystaldba/postgres-mcp
tags: [structured, python, dba, domain-analysis, positioning]
timestamp: 2026-08-09
category: structured
version: 0.3.0
version_checked: 2026-08-09
status: dormant
depth: profile
---

# Postgres MCP Pro (crystaldba)

| Field | Value |
|---|---|
| Repository | <https://github.com/crystaldba/postgres-mcp> |
| Version | **v0.3.0** (2025-05-16) — ~15 months without a release |
| Stars / activity | ~3,173 · last push **2026-01-22** — ~7 months without a commit |
| Language / license | Python · MIT |
| MCP tools | 9 |

## What it does

Not context serving — **DBA expertise as tools**: `list_schemas`, `list_objects`,
`get_object_details`, `execute_sql`, `explain_query`, `get_top_queries`,
`analyze_workload_indexes`, `analyze_query_indexes`, `analyze_db_health`.

The differentiated ones are the analysis tools: index tuning via hypothetical indexes,
`pg_stat_statements` workload analysis, and database health checks.

## The lesson

**In a commoditized category, domain analysis beats data access.** By early 2026 every database
vendor shipped an MCP server that could run SQL and list schemas — see
[the vendor tier](vendor-database-servers.md). Postgres MCP Pro reached ~3.2k stars by shipping
what those servers do not: judgment about the data, not just access to it.

The parallel for Tarn is direct. "Search my notes" is becoming commodity —
[MCPVault](../obsidian-pkm/mcpvault-bitbonsai.md), [lore](../general/lore.md),
[markdown-vault-mcp](../documents/markdown-vault-mcp.md),
[knowledge-rag](../documents/knowledge-rag.md), and
[engraph](../general/engraph.md) all do it. The analysis-shaped equivalents Tarn's index already
has the data for, and nobody exposes:

- **corpus health** — orphaned notes, broken wikilinks, unresolved links, duplicate headings,
  sections that no query ever retrieves;
- **structure analysis** — heading hierarchies that are too deep or too flat, sections far over
  a token budget;
- **coverage gaps** — tags or topics with thin content.

[engraph](../general/engraph.md) ships a `health` tool and PARA migration in this spirit; the
Storks profile's link-graph tooling (orphans, dead ends) is the same instinct. It is
low-hanging fruit because the index already contains everything required.

## Status caution

Seven months without commits and fifteen without a release. Its README's "Related Projects"
section remains the best hand-curated map of the Postgres MCP landscape.
