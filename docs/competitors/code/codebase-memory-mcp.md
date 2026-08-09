---
type: Competitor Profile
title: codebase-memory-mcp
description: Pure-C code knowledge graph with tree-sitter grammars for 158 languages and cross-service HTTP route linking — impressive claims, unusual credibility signals.
resource: https://github.com/DeusData/codebase-memory-mcp
tags: [code, c, knowledge-graph, tree-sitter, cypher, unverified-claims]
timestamp: 2026-08-09
category: code
version: 0.9.1-rc.1
version_checked: 2026-08-09
status: active
depth: profile
---

# codebase-memory-mcp (DeusData)

| Field | Value |
|---|---|
| Repository | <https://github.com/DeusData/codebase-memory-mcp> |
| Version | **v0.9.1-rc.1** (2026-07-30); latest full release v0.9.0 (2026-07-08) — still pre-1.0 |
| Stars / activity | ~38,242 · 3,044 forks · last push 2026-08-09 · **repo created 2026-02-24** |
| Language / license | **C** (no language runtime) · MIT |
| MCP tools | 15 |

## What it does

Tree-sitter grammars for **158 languages** vendored into the binary, plus "Hybrid LSP"
semantic type resolution for 12, building a **persistent knowledge graph** of functions,
classes, call chains, HTTP routes, and cross-service links. Dockerfiles, Kubernetes manifests,
and Kustomize overlays are indexed as graph nodes. Fifteen tools covering search, trace,
architecture, impact analysis, index-coverage checks, **Cypher queries**, dead-code detection,
cross-service HTTP linking, and ADR management. Optional 3D graph UI on `localhost:9749`.

Claims: the Linux kernel (28M LOC / 75K files) indexed in 3 minutes, sub-1ms structural
queries, "83% answer quality", "120x fewer tokens".

## ⚠️ Credibility caveat — read before benchmarking against it

The signals around this project are unusual and the numbers above should be treated as
**unverified marketing** until independently reproduced:

- ~38k stars in roughly 5.5 months, but only **~163 watchers** — a ratio around 10× off what
  a project with that star count normally shows.
- A **self-published arXiv preprint** (2603.27277) cited as its own validation.
- A README dense with self-issued badges (VirusTotal, SLSA 3, "6768 tests passing").
- Still pre-1.0 (`v0.9.1-rc.1`) despite the adoption claims.

None of that proves anything is wrong with the code, and the *technical ideas* are worth
studying. But do not use it as a performance bar in Tarn's own benchmarking without
reproducing its numbers first.

## Ideas worth noting

- **Cross-service HTTP route linking** — resolving an HTTP call in service A to the route
  handler in service B, so the graph spans a microservice fleet. Genuinely novel here.
- **RAM-first indexing pipeline** (LZ4 + in-memory SQLite + fused Aho-Corasick) with memory
  released after indexing.
- **Auto-install into "43 client surfaces"** — writing config for every detected agent
  harness. Install friction matters more for adoption than ranking quality does, and this is
  the most aggressive answer to it in the survey.

## Relevance to Tarn

It is a pure structural graph — no BM25, no prose, no PDFs — so it does not compete on Tarn's
axis. The onboarding automation is the transferable part.
