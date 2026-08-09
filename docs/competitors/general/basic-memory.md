---
type: Competitor Profile
title: basic-memory
description: AGPL Python knowledge server where the agent writes structured markdown back — typed observations and wikilink relations traversed as a graph, with a paid hosted tier over the same engine.
resource: https://github.com/basicmachines-co/basic-memory
tags: [general, python, agpl, open-core, graph-traversal, tool-annotations]
timestamp: 2026-08-09
category: general
version: 0.22.1
version_checked: 2026-08-09
status: active
depth: profile
---

# basic-memory (Basic Machines)

| Field | Value |
|---|---|
| Repository | <https://github.com/basicmachines-co/basic-memory> |
| Version | **v0.22.1** (2026-06-13) |
| Stars / activity | ~3,610 · last push 2026-08-06 |
| Language / license | Python · **AGPL-3.0** |
| Store | SQLite (or Postgres) + FastEmbed vectors; optional Milvus |

## What it does

Plain markdown files with a **semantic structure the agent writes**: typed `[observation]`
categories and `relates_to [[wikilink]]` relations that form a traversable graph. Retrieval
follows links rather than only ranking chunks. Hybrid full-text + vector search with optional
reranking, and **matched chunk text is included in results** so the model gets content, not
just pointers. Interoperates with Obsidian directly — same files.

## Three things worth taking

1. **Per-tool behavior annotations.** Every tool is tagged read-only / destructive /
   idempotent so clients can apply policy without calling. This is an MCP spec feature most
   servers ignore; [lore](lore.md) and
   [Vault as MCP](../obsidian-pkm/vault-as-mcp-ebullient.md) also implement it. Nearly free,
   and it improves client-side safety. Tarn should annotate from the first write tool onward.
2. **Return the matched chunk text inline.** If a search result is only a pointer, the agent
   must make a second call for every hit. Tarn returns sections directly, which is right —
   worth keeping as a deliberate choice, not an accident.
3. **The open-core model, executed cleanly.** Identical engine, identical plain-markdown files,
   local-first and free; the paid tier ($15/mo) hosts the database and sync. The marketing line
   — *"no lock-in — your notes are plain Markdown"* — is credible precisely because the local
   version is the same software. This is the clearest monetization template in the survey, and
   **AGPL-3.0 is the deliberate mechanism that protects it.**

## The licensing note

AGPL-3.0 is an adoption tax for commercial and enterprise users, and basic-memory chose it on
purpose. Together with [Khoj](khoj.md) (also AGPL) it means **a permissive MIT/Apache Rust
binary is a genuine differentiator** in this category, not just a preference. Worth stating in
Tarn's positioning.

## Where Tarn differs

Different retrieval philosophy — graph traversal over agent-authored relations, versus ranked
retrieval over existing structure. basic-memory's corpus is largely *written by the agent*;
Tarn's is *written by you*, and Tarn's job is to find things in what already exists. Both are
valid; they compete for the same "my AI should know my notes" user with different answers.
