---
type: Competitor Profile
title: Context7
description: Cloud service serving up-to-date public library documentation to agents through two tools — the strongest brand in "context for agents", and a positioning foil rather than a rival.
resource: https://github.com/upstash/context7
tags: [documents, typescript, cloud, public-docs, positioning-foil]
timestamp: 2026-08-09
category: documents
version: 4.0.0
version_checked: 2026-08-09
status: active
depth: profile
---

# Context7 (Upstash)

| Field | Value |
|---|---|
| Repository | <https://github.com/upstash/context7> |
| Version | npm `@upstash/context7-mcp` **4.0.0** (2026-08-07) |
| Stars / activity | **~60,466** · last push 2026-08-07 |
| Language / license | TypeScript · MIT |
| MCP tools | **2** — `resolve-library-id`, `get-library-docs` |
| Local-first | **No** — cloud-hosted; API key for higher rate limits |

## What it does

Serves **up-to-date documentation for public libraries** from a hosted service. The agent
resolves a library name to an ID, then fetches current docs for it. That is the entire
surface: two tools, no local indexing, no user corpus.

## Why it is in this bundle

**As a positioning foil, and as evidence about surface size.**

At ~60k stars it is the most-adopted "context for agents" project in existence, and it does
something Tarn explicitly does not: it serves *the world's* documentation, not *yours*. The two
are complementary, and the pairing is worth stating in Tarn's own README:

> **Context7 for the world's docs. Tarn for yours.**

Two further observations:

1. **Two tools, 60k stars.** Together with cognee (3), [ostk-recall](../general/ostk-recall.md)
   (2), [DBHub](../structured/dbhub.md) (2), and [lore](../general/lore.md) (6), this is
   decisive evidence that small tool surfaces do not limit adoption — the outliers in this
   survey are [engraph](../general/engraph.md) at 25 and
   [markdown-vault-mcp](markdown-vault-mcp.md) at 33.
2. **The word "context" is contested.** Context7 owns it in the agent-tooling space, and
   [DataHub and OpenMetadata](../structured/datahub-openmetadata.md) both rebranded to "context
   platform / context layer" in 2026. Tarn should pick positioning language deliberately —
   "retrieval over your own files" is more differentiated and less crowded than "context
   server".

## Relevance to Tarn

Not a competitor. A naming lesson, a surface-size data point, and a complementary tool worth
naming explicitly.
