---
type: Competitor Profile
title: trusty-search
description: Rust machine-wide code search combining BM25, vectors, and a knowledge graph with zero cold-start — one always-on index across every project rather than one per repo.
resource: https://github.com/bobmatnyc/trusty-tools
tags: [general, rust, bm25, machine-wide, zero-cold-start, multi-corpus]
timestamp: 2026-08-09
category: general
version: 0.42.3
version_checked: 2026-08-09
status: active
depth: profile
---

# trusty-search (bobmatnyc/trusty-tools)

| Field | Value |
|---|---|
| Repository | <https://github.com/bobmatnyc/trusty-tools> |
| Version | crates.io `trusty-search` **0.42.3** (release tag 2026-08-05) |
| Downloads | ~2,291 |
| Stars / activity | ~15 · last push 2026-08-09 (active) |
| Language / license | Rust · MIT |

## The idea worth taking

**Machine-wide indexing with zero cold-start.** Instead of one index per repository — which is
what [codanna](../code/codanna.md), [codesearch](../code/codesearch.md), and
[ck](../code/ck.md) all do — trusty-search maintains a **single always-on index spanning every
project on the machine**. There is no "first index this directory" step when you move to a new
project, because it is already indexed.

This maps cleanly onto Tarn's pivot and is arguably the more natural framing of it. The pivot is
usually described as "index more *formats*"; trusty-search suggests the more valuable axis may
be "index more *places*, always". A personal corpus is not one directory — it is a vault, plus
several source trees, plus a papers folder, plus downloaded documentation. A tool that requires
per-directory setup is asked to prove itself repeatedly; one that already has the answer is not.

The cost side is real and worth stating: a machine-wide index means broader disk usage, a
harder story about what is indexed (and therefore what could leak into a response), and a need
for the layered ignore-file semantics [ostk-recall](ostk-recall.md) implements.

Retrieval is BM25 plus vectors plus a knowledge graph.

## Context: the Rust code-search tier

trusty-search sits in a crowded tier of small Rust MCP retrieval crates — `ripvec-mcp`
(4.1.15, ~1,074 downloads, **GitHub repo now 404s while crates.io still publishes**), `frigg`
(0.10.1, ~506 downloads), `rag-rat-mcp` (0.22.0, ~1,617 downloads), `open-kioku`, `RustRAG`,
`cqs`.

**Two structural observations from that tier:**

1. **Download counts are tiny.** [codanna](../code/codanna.md)'s ~12,917 is the outlier; most
   are in the hundreds. **Nobody has won this category** — distribution is wide open.
2. **Essentially all of them index code.** The Rust MCP retrieval ecosystem is thriving on the
   code side and near-empty on the document side, where the only other Rust full-text document
   server found — `Kurogoma4D/file-search-mcp` — is an abandoned toy with an in-memory index
   rebuilt on every run.
