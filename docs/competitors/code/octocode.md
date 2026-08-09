---
type: Competitor Profile
title: Octocode
description: MCP server for searching GitHub at large, notable for exposing response density as an explicit per-call tool parameter.
resource: https://github.com/bgauryy/octocode-mcp
tags: [code, typescript, github, token-density, remote]
timestamp: 2026-08-09
category: code
version: 9.1.1
version_checked: 2026-08-09
status: active
depth: profile
---

# Octocode (bgauryy)

| Field | Value |
|---|---|
| Repository | <https://github.com/bgauryy/octocode-mcp> |
| Version | Latest GitHub release tagged **9.1.1** (2025-12-15), but the latest *tag* is `v5.0.0` — **version numbering is inconsistent; treat as unverified** |
| Stars / activity | ~904 · last push 2026-08-08 (active) |
| Language / license | TypeScript with a Rust engine · MIT |
| MCP tools | 17 total, 14 registered by default |

## What it does

Searches **GitHub at large** — `ghSearchCode`, `ghGetFileContent`, `ghViewRepoStructure`,
`ghSearchRepos`, `ghSearchPullRequests`, `ghSearchIssues`, `ghSearchCommits` — plus local tools
(off by default over MCP, on by default in the CLI) and LSP integration. **Not local-first**:
its value is remote repository search, needing a GitHub token for private repos and subject to
rate limits.

## The one idea worth taking

**Response density as an explicit tool parameter.** Two knobs:

- `concise: true` — path-only listings instead of full results.
- `minify` with three levels:
  - `symbols` — a skeleton with line numbers,
  - `standard` — comments and blank lines stripped (the default),
  - `none` — exact bytes.

Letting the *agent* choose response density per call is underused across this entire survey,
and it is cheap to add. Tarn's natural analogue for a section read is
`heading_path`-only → snippet → full section, selectable per call. It composes with the
metadata-first default that [codesearch](codesearch.md) and
[knowledge-rag](../documents/knowledge-rag.md) converge on: `compact` decides *whether*
content comes back, `minify` decides *how much*.

## Relevance to Tarn

Low as a competitor — different corpus (public GitHub), different trust model (token-gated,
rate-limited). The density parameter is the takeaway.
