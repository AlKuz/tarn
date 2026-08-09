---
type: Competitor Profile
title: Claude Context (Zilliz)
description: TypeScript MCP server doing embeddings-only code search backed by Milvus/Zilliz Cloud — the highest-profile entrant, and cloud-gated.
resource: https://github.com/zilliztech/claude-context
tags: [code, typescript, embeddings, milvus, cloud-dependent, positioning]
timestamp: 2026-08-09
category: code
version: 0.1.15
version_checked: 2026-08-09
status: dormant
depth: profile
---

# Claude Context (zilliztech)

| Field | Value |
|---|---|
| Repository | <https://github.com/zilliztech/claude-context> |
| Version | npm `@zilliz/claude-context-mcp` **0.1.15** (2026-06-22); repo tag `v0.1.11`, no GitHub releases |
| Stars / activity | ~12,343 · last push **2026-07-14** — ~4 weeks stale at review |
| Language / license | TypeScript · MIT |
| Local-first | **No** — requires `MILVUS_ADDRESS` + `MILVUS_TOKEN` and an `OPENAI_API_KEY` |

## What it does

Embeddings-only semantic search over code chunks, stored in **Milvus / Zilliz Cloud**. No BM25,
no lexical lane. Ships a VS Code Marketplace extension (`zilliz.semanticcodesearch`) alongside
the MCP server.

## Why it is in this bundle

**As a positioning lesson, not a technical one.** This is the highest-profile entrant in the
code category by stars, and it is:

- an API-key-gated wrapper over a hosted vector database (a Zilliz Cloud funnel),
- embeddings-only, with no keyword lane at all,
- slowing — the README now banners a sibling project (`zilliztech/memsearch`).

Twelve thousand stars for that configuration says the market rewards **a crisp name and a
one-line install** more than it rewards architecture. "Claude Context" is effectively
category name-squatting.

Two conclusions for Tarn:

1. **"Zero API keys, zero external services" is a genuine, checkable differentiator** against
   the most visible competitor in the space. Say it plainly.
2. **Naming and install friction matter more than they should.** A single
   `claude mcp add`-able binary with an obvious name beats a better-engineered tool that is
   harder to try.

Its slowing cadence is also an opening.
