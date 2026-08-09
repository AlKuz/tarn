---
type: Competitor Profile
title: mcp-obsidian (MarkusPfundstein)
description: The most-adopted Obsidian MCP server by a wide margin — a thin REST proxy with no index, keyword-only search, and a stale release cadence.
resource: https://github.com/MarkusPfundstein/mcp-obsidian
tags: [obsidian-pkm, python, rest-proxy, no-index, adoption-leader]
timestamp: 2026-08-09
category: obsidian-pkm
version: 0.2.2
version_checked: 2026-08-09
status: dormant
depth: profile
---

# mcp-obsidian (MarkusPfundstein)

> **This project was missing from the 2026-03 review, and it is the category's actual adoption
> leader** — roughly 6.5× the stars of the next Obsidian MCP server. Its absence made the
> earlier competitive picture materially wrong.

| Field | Value |
|---|---|
| Repository | <https://github.com/MarkusPfundstein/mcp-obsidian> |
| Version | PyPI **0.2.2** (2025-04-01) — no GitHub releases |
| Stars / activity | **~4,279** · last push 2026-05-15 |
| Language / license | Python · MIT |
| Vault access | Proxies the **Obsidian Local REST API** plugin |

## What it does

Proxies the [Local REST API](https://github.com/coddingtonbear/obsidian-local-rest-api) plugin
(~2,763★, itself now shipping an MCP server). It requires Obsidian to be **running**, has **no
index of its own**, and offers **keyword-only search** through the REST endpoint.

That is the whole architecture. It is the thinnest serious project in the category.

## Why it matters

**The most-used Obsidian MCP server has no index, no ranking, and no section awareness — and
it has not shipped a release since April 2025.**

Two conclusions follow, and they point in opposite directions:

1. **The bar for "useful" is far lower than the engineering in this bundle assumes.** Users
   adopted a keyword proxy over servers with BM25 ([MCPVault](mcpvault-bitbonsai.md), 1,599★),
   hybrid RRF ([engraph](../general/engraph.md), 164★), and HyDE plus reranking
   ([obsidian-tools](obsidian-tools-glibalien.md), 2★). Retrieval quality is not what drives
   adoption here — being early, obvious, and easy to install is.
2. **It is a genuinely soft target.** Requires the app running, keyword-only, dormant for
   fifteen months on PyPI. Every structural advantage Tarn has — headless operation, a
   persistent ranked index, section-level retrieval — is a direct answer to a real limitation
   in the incumbent, not a hypothetical improvement.

The tension between those two is the honest read: the incumbent is beatable on capability, but
capability is not what won it the position.

## Relevance to Tarn

Keep it in the comparison table as the **adoption baseline** for the Obsidian category, the way
[the official filesystem MCP server](../general/mcp-reference-servers.md) is the baseline for
the general category. Comparing Tarn against sophisticated-but-unused competitors flatters the
analysis; comparing it against what people actually run does not.
