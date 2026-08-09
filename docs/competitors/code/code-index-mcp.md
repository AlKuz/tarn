---
type: Competitor Profile
title: Code Index MCP
description: Python code indexer with a two-tier shallow/deep index, self-describing escalation errors, and search delegated to the best available native binary.
resource: https://github.com/johnhuang316/code-index-mcp
tags: [code, python, tree-sitter, two-tier-index, ripgrep, escalation]
timestamp: 2026-08-09
category: code
version: 2.17.1
version_checked: 2026-08-09
status: active
depth: profile
---

# Code Index MCP (johnhuang316)

| Field | Value |
|---|---|
| Repository | <https://github.com/johnhuang316/code-index-mcp> |
| Version | **v2.17.1** (2026-07-27), PyPI identical |
| Stars / activity | ~995 · last push 2026-07-27 |
| Language / license | Python · MIT |
| MCP tools | ~10 |

## What it does

Dual-strategy indexing: tree-sitter AST parsing for 10 core languages (Python, JS, TS, Java,
Kotlin, C#, Go, Objective-C, Zig, Rust) with fallback file indexing for 50+ others. Search
**delegates to the best available native binary** — `ugrep` → `ripgrep` → `ag` → `grep`,
auto-detected at runtime. Persistent cache in a temp directory; stdio transport. Ships
`.well-known/mcp.json` and `.well-known/mcp.llmfeed.json` discovery manifests.

Notably it lists SQL variants, stored procedures, and migrations among indexed types —
overlapping the structured-data ambitions of Tarn's pivot.

## Ideas worth taking

- **The two-tier index.** `refresh_index` does a shallow, fast file discovery pass;
  `build_deep_index` does the expensive symbol-level pass **on demand**. Startup is cheap and
  you only pay for depth when something needs it. For Tarn this maps cleanly: a fast pass
  building the section skeleton (paths, `heading_path`, titles), and a deeper pass computing
  full term statistics or per-format extraction for large binary formats.
- **`needs_deep_index` as a self-describing response field.** Tools return a flag telling the
  agent the index is too shallow to answer and it should escalate. This is the same family as
  [codanna](codanna.md)'s guidance templates and
  [cyanheads](../obsidian-pkm/obsidian-mcp-server-cyanheads.md)'s `recovery.hint`: the failure
  carries its own remedy, so no human has to intervene.
- **Literal-by-default search with opt-in regex**, fuzzy matching, and pagination at 10 per
  page.
- **`.well-known/` discovery manifests** — a low-cost distribution win.

## Caution

Do not confuse this project with the **ViperJuice / Consiliency "Code-Index-MCP" forks** that
appear in search results claiming 25+ tools, CozoDB, and Voyage AI. Those are different,
lower-quality projects; the Consiliency npm package was unpublished.
