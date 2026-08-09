---
type: Competitor Profile
title: zotero-mcp
description: Zotero research-library MCP server with ~4.6k stars — evidence that a narrow, well-served corpus beats a general one for adoption.
resource: https://github.com/54yyyu/zotero-mcp
tags: [documents, python, zotero, vertical, adoption-lesson]
timestamp: 2026-08-09
category: documents
version: 0.9.1
version_checked: 2026-08-09
status: active
depth: profile
---

# zotero-mcp (54yyyu)

| Field | Value |
|---|---|
| Repository | <https://github.com/54yyyu/zotero-mcp> |
| Version | **v0.9.1** (2026-08-06) |
| Stars / activity | **~4,578** · last push 2026-08-06 |
| Language / license | Python · MIT |

## What it does

Exposes a **Zotero** research library — papers, PDFs, annotations, and citation metadata — to
agents over MCP.

## Why it is in this bundle

**Adoption evidence, and it is the uncomfortable kind.**

~4,578 stars for a single-corpus, single-application connector. Compare, in the same review:

| Project | Scope | Stars |
|---|---|---|
| zotero-mcp | One app's library | **~4,578** |
| [markdown-vault-mcp](markdown-vault-mcp.md) | Any markdown vault, adaptive chunking, hybrid RRF | ~27 |
| [lore](../general/lore.md) | 90+ formats, 10 source types, Rust, Tantivy | ~17 |
| [ostk-recall](../general/ostk-recall.md) | Multi-corpus, hybrid, daemon | ~6 |

**A narrow, well-served corpus out-adopts a general one by two orders of magnitude.** The
general-purpose projects are more capable and almost unused.

The plausible reason: a vertical connector answers "what is this for?" in four words, and its
setup has exactly one step. A general-purpose retrieval server asks the user to decide what to
index, how to configure sources, and why it beats grep — before they see any value.

## What this means for Tarn's pivot

The pivot is directionally right, but this is the risk it carries, and the analysis should say
so plainly. Two mitigations both visible in this survey:

- **Ship named presets, not just a generic config.** [lore](../general/lore.md)'s `lore init`
  auto-detects doc folders and writes a pre-filled config; `knowledge-rag` ships a `presets/`
  directory. A `tarn init --preset obsidian` / `--preset rust-project` / `--preset docs-site`
  turns a general tool back into a one-step vertical one at the point of first use.
- **Keep Obsidian as a first-class, named use case.** It is where the search volume and the
  existing audience are. General-purpose capability should be an expansion of the pitch, not a
  replacement for it.
