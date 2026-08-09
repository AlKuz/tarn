---
type: Competitor Profile
title: Sourcebot
description: Self-hosted multi-repo code search for teams, with a web UI, auth, and Docker-compose deployment — the enterprise end of the category.
resource: https://github.com/sourcebot-dev/sourcebot
tags: [code, typescript, self-hosted, multi-repo, team, non-standard-license]
timestamp: 2026-08-09
category: code
version: 5.1.5
version_checked: 2026-08-09
status: active
depth: profile
---

# Sourcebot (sourcebot-dev)

| Field | Value |
|---|---|
| Repository | <https://github.com/sourcebot-dev/sourcebot> |
| Version | **v5.1.5** (2026-07-31) |
| Stars / activity | ~3,659 · last push 2026-08-08 |
| Language | TypeScript |
| License | ⚠️ **NOASSERTION** — GitHub cannot identify a standard license. **Verify licensing before any comparison or code reuse; this is not plain OSS.** |

## What it does

Self-hosted, Docker-compose deployed code search serving "humans and agents". A JSON config
file declares which repositories to index, which LLM providers to use, and which auth
providers to accept. Ships a web UI.

Its search core is widely described as Zoekt-style trigram indexing — **unverified**, inferred
from lineage rather than confirmed in the source read for this review.

## Relevance to Tarn

**Low as a direct competitor, useful as a boundary marker.** Sourcebot sells to a *platform
team*: multi-repo, SSO, a web UI, a deployment story. Tarn sells to an individual developer or
agent operator: one binary, one config, no infrastructure.

Two things worth carrying into the analysis:

1. **It marks the ceiling of where a local tool could expand** — and why it probably should
   not. Every capability on that list (auth providers, web UI, compose stack) is a permanent
   tax on the single-binary value proposition.
2. **The non-standard license is a reminder to check before benchmarking.** Several projects
   in this space have licensing that is not what a badge implies —
   [obsidian-tools](../obsidian-pkm/obsidian-tools-glibalien.md) and
   [obsidian-mcp (Storks)](../obsidian-pkm/obsidian-mcp-storks.md) both have README license
   claims with no LICENSE file behind them.
