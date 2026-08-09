---
type: Competitor Profile
title: XiYan-SQL MCP Server
description: The one structured-data server that actually does retrieval — embedding-based pruning over tables, columns, and values before SQL generation.
resource: https://github.com/XGenerationLab/xiyan_mcp_server
tags: [structured, python, schema-rag, text-to-sql, retrieval, alibaba]
timestamp: 2026-08-09
category: structured
version: 0.1.4
version_checked: 2026-08-09
status: dormant
depth: profile
---

# XiYan-SQL MCP Server (XGenerationLab / Alibaba)

| Field | Value |
|---|---|
| Repository | <https://github.com/XGenerationLab/xiyan_mcp_server> |
| Version | **v0.1.4** (2025-03-24) |
| Stars / activity | ~241 · last push **2026-02-11** — ~6 months stale |
| Language / license | Python · Apache-2.0 |
| MCP tools | Essentially one — `get_data` (natural language in, rows out) plus a sample-data resource |
| Local-first | ⚠️ Hybrid — default remote mode needs an API key for a 32B model; local mode runs a 3B model on-device |

## Why it matters

**This is the only server in the structured category that does retrieval at all**, and it does
it with the exact primitive Tarn is built on.

Its "Multi-path Retrieval" prunes large schemas by computing **cosine similarity over embeddings
of tables, columns, and values**, selecting the relevant subset before generating SQL. Backed by
a paper ([arXiv 2507.04701](https://arxiv.org/pdf/2507.04701)) with state-of-the-art text-to-SQL
benchmark results.

**The generalization for Tarn's pivot:** a table, a column, and a view are *chunks*. They have
identity, hierarchy (`database → schema → table → column`), and text worth indexing (names,
types, comments, sample values). If Tarn adds SQL as a source kind, the right model is not "run
queries" — every other server in this category already does that, and it is commoditized. The
right model is **schema elements as sections in the same ranked index**, with the hierarchy
playing the role `heading_path` plays for markdown.

That would make Tarn the only server able to answer "where in my systems is customer email
stored?" across a database schema, a design document, and the code that touches it — in one
ranked result set. Nothing in this survey does that.

**And note the tool count: one.** Natural language in, rows out.

## Caution

Six months without a commit, still v0.1.x, and the default configuration is API-key gated. The
*idea* is the contribution; the implementation is not a benchmark target.
