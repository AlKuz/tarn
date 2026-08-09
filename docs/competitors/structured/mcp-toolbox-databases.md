---
type: Competitor Profile
title: MCP Toolbox for Databases (Google)
description: Google's config-as-tools database gateway — a generic runtime where tools are hand-authored YAML, and the category leader by adoption.
resource: https://github.com/googleapis/mcp-toolbox
tags: [structured, go, config-as-tools, google, category-leader]
timestamp: 2026-08-09
category: structured
version: 1.8.0
version_checked: 2026-08-09
status: active
depth: profile
---

# MCP Toolbox for Databases (googleapis)

| Field | Value |
|---|---|
| Repository | <https://github.com/googleapis/mcp-toolbox> — ⚠️ **renamed** from `googleapis/genai-toolbox`; the old path 404s |
| Version | **v1.8.0** (2026-07-28) |
| Stars / activity | ~16,138 · last push 2026-08-08 |
| Language / license | **Go** · Apache-2.0 · SDKs for Python, JS, Go, Java |
| Databases | The broadest matrix in the category — AlloyDB, BigQuery, Spanner, Postgres, MySQL, SQLite, Neo4j, and more |

## What it does

A **generic runtime whose tools are declarative YAML**. You hand-author parameterized SQL and it
becomes an MCP tool. There is no retrieval and no ranking — the *curation* is the product.

Roughly 28 tools in the default configuration, which is the figure
[DBHub](dbhub.md) benchmarks against on token cost (19.0k vs 1.4k).

## Relevance to Tarn

**The shape, not the domain.** "One binary, one config file, many backends" is exactly what
Tarn's pivot needs to become, and this is the most-adopted implementation of that shape in the
MCP ecosystem. Google-backed, Apache-2.0, 16k stars.

Two lessons pointing in opposite directions, and both are real:

- **Config-as-tools works.** A declarative file that turns a data source into a callable
  capability is a proven, popular model. Tarn's `[[sources]]` config (see
  [ostk-recall](../general/ostk-recall.md) for the reference schema) is the same idea applied to
  corpora rather than queries.
- **A 28-tool default is a documented liability.** Competitors attack it directly and
  quantitatively. Tarn should ship a deliberately small tool count and be prepared to say what
  it costs in tokens — the number is the argument.

The renaming is also a practical caution for this bundle: `genai-toolbox` → `mcp-toolbox` broke
every existing link. Three projects in this survey renamed during the review window
([Kreuzberg → Xberg](../general/xberg.md),
[obsidian-vault-mcp → Vault as MCP](../obsidian-pkm/vault-as-mcp-ebullient.md),
[mcp-obsidian → MCPVault](../obsidian-pkm/mcpvault-bitbonsai.md)), which is why every profile
here records the former name.
