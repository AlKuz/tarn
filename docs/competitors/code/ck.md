---
type: Competitor Profile
title: ck (seek)
description: Rust grep-compatible search combining BM25 and local embeddings with RRF, chunk-level blake3 incremental indexing, and MCP cursor pagination.
resource: https://github.com/BeaconBay/ck
tags: [code, rust, bm25, rrf, incremental-index, blake3, grep-compatible]
timestamp: 2026-08-09
category: code
version: 0.7.11
version_checked: 2026-08-09
status: active
depth: profile
---

# ck ("seek") — BeaconBay

| Field | Value |
|---|---|
| Repository | <https://github.com/BeaconBay/ck> |
| Version | crates.io `ck-search` **0.7.11** (2026-05-24) |
| Stars / activity | ~1,689 · last push 2026-08-04 |
| Language / license | Rust · Apache-2.0 (MIT/Apache dual per README) |
| MCP tools | 6 — `semantic_search`, `regex_search`, `hybrid_search`, `index_status`, `reindex`, `health_check` |

## What it does

BM25 keyword search plus local embeddings, fused with **Reciprocal Rank Fusion** (`--hybrid`).
`--full-section` returns the entire enclosing function or class rather than a matching line.
The on-disk index builds lazily and refreshes transparently on first search. Served over stdio
via `ck --serve`.

## Ideas worth taking

- **Chunk-level blake3 content hashing for incremental reindex.** ck hashes
  `blake3(text + trivia)` per chunk, claiming 80–90% cache hit rates on typical edits, and
  correctly invalidates when only a doc comment or whitespace changed. **This is directly
  applicable to Tarn's index sync**: hashing at *section* granularity rather than file
  granularity means editing one section of a large note reindexes one section. Tarn already
  has `RevisionToken` (a content hash) at note level — extending the same idea per section is
  a natural fit.
- **Model-consistency guarding.** ck detects when the embedding model has changed and refuses
  to serve a mismatched index rather than silently returning garbage. Any Tarn feature with a
  versioned artifact (tokenizer, scorer, schema) needs the same guard —
  [ostk-recall](../general/ostk-recall.md) requires a manual `init --force` for the same
  situation, which is worse UX.
- **MCP cursor pagination with snippet-length control.** Tarn will need this the moment a
  corpus is large enough that a result page cannot hold the matches.
- **Drop-in grep compatibility** (`-i -n -A -B -l -L -R --exclude` behave as in grep) so one
  binary serves humans and agents. A `--tui` mode adds an interactive relevance heatmap.

## Where Tarn differs

ck has no structural layer — no heading hierarchy, no section path, no link graph.
`--full-section` returns the enclosing syntactic block but nothing is *ranked* by structure.
Tarn's `heading_path` is both a ranking signal and an addressing scheme that ck has no
equivalent for.
