---
type: Competitor Profile
title: kb-mcp-server (txtai)
description: txtai-based knowledge-base server whose distinguishing idea is a portable, shippable index artifact — build once, hand over a single .tar.gz.
resource: https://github.com/Geeksfino/kb-mcp-server
tags: [documents, python, txtai, portable-index, knowledge-graph]
timestamp: 2026-08-09
category: documents
version: 0.1.3
version_checked: 2026-08-09
status: dormant
depth: profile
---

# kb-mcp-server / "Embedding MCP Server" (Geeksfino)

| Field | Value |
|---|---|
| Repository | <https://github.com/Geeksfino/kb-mcp-server> |
| Version | GitHub releases stop at **v0.1.3** (2025-05-20) while docs reference PyPI 0.3.0 — **current version unverified** |
| Stars / activity | ~71 · last push 2026-07-13 |
| Language / license | Python · MIT |
| Built on | [neuml/txtai](https://github.com/neuml/txtai) (~12,816★, active) |

## The idea worth taking

**Knowledge bases as portable `.tar.gz` archives.** Build an index once, hand the file to
someone (or to a server), and it works. No re-ingestion, no re-embedding, no shared database.

This is a genuinely underexploited idea and it maps directly onto Tarn:

- **A team could share a prebuilt index of a documentation set** — one artifact, no per-machine
  ingestion cost, no requirement that every consumer can even read the source files.
- **CI could build an index as a release artifact**, so agents consume a versioned, immutable
  corpus snapshot rather than whatever happens to be on disk.
- **Reproducibility**: a bug report against retrieval quality can ship the exact index.

Tarn already persists its index (JSON today, per-component `Persistable` artifacts in a
folder). Making that folder a portable, versioned bundle with a manifest — index format
version, tokenizer identity, source manifest, content hashes — is a small step from where it
already is.

Two guards are required for it to be safe, both of which other projects in this survey supply
the pattern for:

- **A model/tokenizer consistency check on load**, as [ck](../code/ck.md) does, so an index
  built with a different tokenizer is rejected rather than silently mis-ranking.
- **A format-version field**, so an old bundle fails loudly.

Also present: txtai-backed knowledge-graph construction.

## Relevance to Tarn

Low as a competitor (71 stars, unclear release state). The portable-index-artifact concept is
the reason to keep the profile.
