---
type: Competitor Profile
title: Serena
description: Python MCP server wrapping Language Server Protocol backends for 40+ languages, offering symbolic navigation and precise refactoring with no index at all.
resource: https://github.com/oraios/serena
tags: [code, python, lsp, no-index, symbolic-editing, tool-gating]
timestamp: 2026-08-09
category: code
version: 1.6.1
version_checked: 2026-08-09
status: active
depth: profile
---

# Serena (oraios)

| Field | Value |
|---|---|
| Repository | <https://github.com/oraios/serena> |
| Version | v1.6.1 (2026-07-21); PyPI `serena-agent` |
| Stars / activity | ~27,771 · last push 2026-08-08 |
| Language / license | Python · MIT |
| Index | **None** |

## What it does

Wraps **Language Server Protocol** servers for 40+ languages behind an agent-friendly
abstraction, plus an optional JetBrains IDE backend. Symbols come from the LSP live — there is
no precomputed store, no embeddings, and no ranking.

Roughly 25–30 tools across `symbol_tools` (`find_symbol`, `find_declaration`,
`find_implementations`, `find_referencing_symbols`, `get_symbols_overview`,
`get_diagnostics_for_file`), **editing tools** (`replace_symbol_body`, `insert_before_symbol`,
`insert_after_symbol`, `rename_symbol`, `safe_delete_symbol`), file and "memory" tools, and a
JetBrains-only debug REPL. **Most are disabled by default** and enabled per project config.

## Why it matters to Tarn

**Serena has ~27.8k stars with no index, no ranking, and no retrieval quality story at all.**
That is the single most important data point in the code category: this market rewards *tool
ergonomics* far more than *information-retrieval sophistication*. Every project in this bundle
with better retrieval has an order of magnitude fewer users.

Three transferable practices:

- **Tool gating per project.** Serena ships many tools but exposes a curated subset, on the
  reasoning that the host harness already has grep and read. Tarn should assume the same: the
  agent already has file tools, so expose only what genuinely beats them.
- **A published evaluation methodology** with per-harness results (Claude Code, Codex,
  Copilot CLI, Junie) measuring value added *on top of* the harness's built-in tools. That is
  the only honest benchmark for an MCP server, and Tarn should copy the framing — its baseline
  is filesystem MCP plus the agent's own grep.
- **Symbolic editing.** Precise LSP-backed refactors (cross-file rename, move,
  replace-symbol-body) instead of lossy search-and-replace. Tarn's analogue is
  section-addressed writes keyed on `heading_path`.

## Where Tarn differs

Serena is code-only and index-free. It cannot serve prose, PDFs, notes, or SQL, and it cannot
rank anything — you must already know roughly what you are looking for. Tarn's ranked,
persistent, heterogeneous corpus is orthogonal, not competing. The lesson to take is about
*surface design and evaluation*, not architecture.
