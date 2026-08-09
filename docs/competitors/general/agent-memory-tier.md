---
type: Competitor Profile
title: Agent-memory tier (mem0, Graphiti/Zep, Letta)
description: The 100k-star memory category that indexes what the agent said rather than what you wrote — not competitors, but a serious category-confusion risk.
resource: https://github.com/mem0ai/mem0
tags: [general, memory, positioning-risk, mem0, graphiti, letta]
timestamp: 2026-08-09
category: general
version: various
version_checked: 2026-08-09
status: active
depth: profile
---

# Agent-memory tier — mem0, Graphiti (Zep), Letta

| Project | Stars | Latest | Activity | License |
|---|---|---|---|---|
| [mem0ai/mem0](https://github.com/mem0ai/mem0) | ~62,850 | monorepo tags; core version **unverified** | 2026-08-07 | Apache-2.0 |
| [getzep/graphiti](https://github.com/getzep/graphiti) | ~29,695 | v0.29.3 (2026-07-27) | 2026-08-07 | Apache-2.0 |
| [letta-ai/letta](https://github.com/letta-ai/letta) | ~24,156 | 0.16.8 (2026-05-14) | 2026-08-01 | Apache-2.0 |

## What they do

Store **conversation-derived** memory — facts extracted from agent turns — in vector and graph
stores. Graphiti adds bi-temporal edges; Letta adds self-editing agent memory blocks. All are
self-hostable, but the extraction pipelines require an LLM, and mem0 and Zep push toward hosted
platforms.

## Why they are in this bundle

**Not as competitors — as a positioning risk, and it is the largest one Tarn faces.**

They do not index your files. They index what the agent said. But between them they hold well
over 100,000 stars and they have **colonized the word "memory"** in agent tooling. When a user
asks "what should my agent remember?", these are the answers they find.

The vocabularies are actively merging: [cognee](cognee.md) (`remember`/`recall`/`forget`) and
[ostk-recall](ostk-recall.md) (`recall`/`remember`) both adopted memory verbs for what are
fundamentally retrieval tools.

**Tarn should position deliberately as "retrieval over your existing artifacts", not
"memory."** The distinction is real and defensible:

| | Agent memory | Tarn |
|---|---|---|
| Corpus origin | Written by the agent | Written by you |
| Ground truth | The extraction is the record | Files on disk are the record |
| Failure mode | Hallucinated or stale facts | Stale index (detectable, fixable) |
| Requires an LLM | Yes, for extraction | No |
| Verifiable | Hard | Every hit cites a file and section |

Being benchmarked against mem0 on memory-recall accuracy would be losing on the wrong axis
entirely.

## The genuine overlap

**Agent session logs.** [ostk-recall](ostk-recall.md) treats Claude Code, Gemini, and Codex
session logs as first-class source kinds, and two other Rust projects (`callimachus` ~33★,
`sessiongrep` ~29★) do only that. Session logs are files on disk with structure — exactly
Tarn's kind of corpus — and indexing them is where retrieval and memory legitimately meet. It
is a cheap source kind to add and there is no established winner.

One caution carried from ostk-recall: session logs routinely contain pasted secrets. Any such
source kind needs redaction or an explicit warning.
