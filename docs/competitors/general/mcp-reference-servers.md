---
type: Competitor Profile
title: MCP reference servers (filesystem, fetch, memory)
description: The official baseline every agent already has — filesystem plus the agent's own grep is what Tarn must actually beat, and the honest benchmark target.
resource: https://github.com/modelcontextprotocol/servers
tags: [general, typescript, baseline, official, filesystem]
timestamp: 2026-08-09
category: general
version: 2026.7.10
version_checked: 2026-08-09
status: active
depth: profile
---

# MCP reference servers (modelcontextprotocol)

| Field | Value |
|---|---|
| Repository | <https://github.com/modelcontextprotocol/servers> |
| Release | **2026.7.10** (2026-07-10) |
| Stars / activity | **~89,360** · last push 2026-08-05 |
| Language | TypeScript |

## The current set

Verified active: `everything`, `fetch`, `filesystem`, `git`, `memory`, `sequentialthinking`,
`time`. Everything else — including the official `postgres` and `sqlite` servers — has moved to
[`modelcontextprotocol/servers-archived`](https://github.com/modelcontextprotocol/servers-archived),
which is formally archived (last push 2025-05-28).

- `filesystem` — read / write / list / glob. **No index, no ranking, no chunking.**
- `memory` — a trivial JSON knowledge graph.
- `fetch` — URL → markdown.

## Why this is the most important profile in the bundle

**This is what Tarn is actually competing with.** Not [lore](lore.md), not
[engraph](engraph.md), not [markdown-vault-mcp](../documents/markdown-vault-mcp.md) — those
have 17, 164, and 27 stars. Every agent user already has `filesystem` plus a built-in grep, and
that combination is the default state of the world.

Two consequences for how Tarn should be evaluated and pitched:

1. **Benchmarks must target this baseline.** "Tarn versus filesystem MCP + grep" on
   tokens-per-correct-answer and time-to-answer is the only comparison a user cares about.
   Beating a boutique competitor on MRR@5 persuades nobody who has not heard of it.
   [Serena](../code/serena.md) publishes exactly this framing — value added *on top of* the
   harness's built-in tools — and it is the right model.
2. **The "why do I need this?" objection is legitimate and must be answered concretely.**
   Agents grep effectively. Tarn's answer has to be specific: ranked retrieval across a corpus
   too large to grep exhaustively, structure (`heading_path`, tags, links) that grep cannot
   recover, and bounded token cost per query. [codesearch](../code/codesearch.md) answers it
   honestly by telling agents when to use grep instead — a credibility move worth copying.

## The signal in the archiving

Anthropic **shrank** the reference set to seven primitives and archived every vendor and
database server. That is a statement about what belongs in core versus third-party: **indexed
retrieval is explicitly third-party territory.** There is no official baseline in the retrieval
category, which is exactly why it is contested.

The archived `postgres` server carries one more lesson, recorded in
[Postgres MCP Pro](../structured/postgres-mcp-pro.md)'s README: it exposed schema as **MCP
resources**, and the ecosystem rejected the approach because client support for resources is
thin. **If Tarn exposes sections as resources rather than tools, reconsider.**

## Related

`mamertofabian/mcp-everything-search` (~351★, MIT, Python) wraps Windows Everything / macOS
`mdfind` / Linux `locate` for **filename-only** search — last push 2025-10-20, ~10 months
stale. Filename search is the other thing users already have.
