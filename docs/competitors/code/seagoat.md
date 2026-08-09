---
type: Competitor Profile
title: SeaGOAT
description: The original local-first semantic code search engine, predating the MCP era — historical evidence that integration, not retrieval quality, made this category a market.
resource: https://github.com/kantord/SeaGOAT
tags: [code, python, semantic-search, historical, pre-mcp]
timestamp: 2026-08-09
category: code
version: 0.54.17
version_checked: 2026-08-09
status: dormant
depth: profile
---

# SeaGOAT (kantord)

| Field | Value |
|---|---|
| Repository | <https://github.com/kantord/SeaGOAT> |
| Version | **v0.54.17** (2025-05-14) — no release in ~15 months |
| Stars / activity | ~1,302 · last push 2026-07-21 (maintenance only) |
| Language / license | Python · MIT |
| Created | 2023-06 |

## Why it is here

SeaGOAT is the **original** "local-first semantic code search engine", created two years before
the MCP-era projects that now dominate this category. It is primarily a CLI and server, not
MCP-native.

Its trajectory is the useful part: 1,302 stars accumulated over three years, no release in
fifteen months, while MCP-native entrants created in 2026 —
[codanna](codanna.md) (722★ in one year), [claude-context](claude-context-zilliz.md) (12.3k★),
[Serena](serena.md) (27.8k★) — went far past it.

**The conclusion for Tarn:** standalone local semantic search has a low ceiling as a product.
What turned this into a real market in 2025–26 was the **integration surface** — being callable
by an agent — not the retrieval technology, which SeaGOAT had first. Tarn's MCP surface is not
a feature alongside the index; it is the reason the index is worth building.
