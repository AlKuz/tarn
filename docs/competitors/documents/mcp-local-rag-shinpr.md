---
type: Competitor Profile
title: mcp-local-rag (shinpr)
description: TypeScript local-first RAG server with semantic chunking that preserves code blocks intact, aimed at code and technical documentation.
resource: https://github.com/shinpr/mcp-local-rag
tags: [documents, typescript, local-first, semantic-chunking, technical-docs]
timestamp: 2026-08-09
category: documents
version: 0.17.3
version_checked: 2026-08-09
status: active
depth: profile
---

# mcp-local-rag (shinpr)

| Field | Value |
|---|---|
| Repository | <https://github.com/shinpr/mcp-local-rag> |
| Version | **v0.17.3** (2026-08-03) |
| Stars / activity | ~361 · last push 2026-08-05 |
| Language / license | TypeScript · MIT |
| Local-first | Yes — "zero setup", fully private |

## What it does

A local-first RAG MCP server aimed at **code and technical documentation** rather than personal
notes. Hybrid semantic + keyword retrieval, with local persistence and no external services.
Ships both an MCP server and a CLI from one package.

Among the genuinely local-first small players it has the **highest star count** (~361), ahead of
[markdown-vault-mcp](markdown-vault-mcp.md) (27) and [lore](../general/lore.md) (17) — despite
being architecturally less ambitious than either. Positioning and packaging again outrunning
engineering depth.

## The idea worth taking

**Semantic chunking that preserves Markdown code blocks intact.** The chunker finds topic
boundaries but treats fenced code blocks as atomic — a chunk boundary never falls inside a
fence.

This is the constructive counterpart to the defensive fence handling seen elsewhere:
[knowledge-rag](knowledge-rag.md) and
[cyanheads](../obsidian-pkm/obsidian-mcp-server-cyanheads.md) mask fences so a `#` comment is
not mistaken for a heading; shinpr additionally guarantees the *content* of a fence is never
split across chunks.

For Tarn both matter, and for the same reason: technical notes are full of code. A section
split mid-fence returns syntactically invalid code to the agent, which is worse than returning
less. Tarn's section splitter needs an explicit invariant here, and it needs a test.

## Relevance to Tarn

Direct but shallow overlap — same problem space, less structural sophistication. The
code-block-atomicity invariant is the concrete takeaway.
