---
type: Competitor Profile
title: Onyx (formerly Danswer)
description: Enterprise search over 40+ SaaS connectors with permission-aware retrieval — the team/SaaS end of the market, and the best connector abstraction in open source.
resource: https://github.com/onyx-dot-app/onyx
tags: [general, python, enterprise, connectors, permission-aware, boundary]
timestamp: 2026-08-09
category: general
version: 4.5.4
version_checked: 2026-08-09
status: active
depth: profile
---

# Onyx (onyx-dot-app, formerly Danswer)

| Field | Value |
|---|---|
| Repository | <https://github.com/onyx-dot-app/onyx> |
| Version | **v4.5.4** (2026-08-07) |
| Stars / activity | ~31,510 · last push 2026-08-09 |
| Language / license | Python · ⚠️ **NOASSERTION** — non-standard license, verify before comparison |
| Local-first | ❌ in practice — Docker/K8s stack with Postgres + Vespa |

## What it does

Enterprise search and chat over **40+ SaaS connectors** (Slack, Drive, Confluence, Jira,
GitHub, …) with **permission-aware retrieval**, layered with MCP support. Deployed as an
infrastructure stack, not a binary.

## Relevance to Tarn

**Low as a competitor; it marks the other boundary.** Onyx owns *team and SaaS* context; Tarn
owns *personal and local-disk* context. The buyers are different and the deployment models are
incompatible.

**One thing genuinely worth studying: the connector abstraction.** Onyx's is the most
battle-tested "N heterogeneous sources, one index" design in open source — forty-plus
connectors, each with its own auth, pagination, incremental-sync semantics, and permission
model, all feeding one retrieval layer. Tarn's pivot needs the same shape at smaller scale, and
the hard-won details (incremental cursors, deletion detection, per-source failure isolation)
are visible there.

**What not to follow:** permission-aware multi-tenancy. Onyx must model who may see which
document because its corpus spans an organization. Tarn's corpus is one user's disk, where the
filesystem already answers that question. Adopting per-document ACLs would import enormous
complexity for a problem Tarn does not have — path-scoped read/write limits of the kind
[cyanheads](../obsidian-pkm/obsidian-mcp-server-cyanheads.md) and
[Vault as MCP](../obsidian-pkm/vault-as-mcp-ebullient.md) implement are the right ceiling.
