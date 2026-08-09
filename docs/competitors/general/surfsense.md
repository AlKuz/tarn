---
type: Competitor Profile
title: SurfSense
description: Open-source NotebookLM alternative researching the live web through one platform, API, or MCP server — web-first rather than disk-first.
resource: https://github.com/MODSetter/SurfSense
tags: [general, python, web-research, notebooklm, triple-surface]
timestamp: 2026-08-09
category: general
version: 0.0.36
version_checked: 2026-08-09
status: active
depth: profile
---

# SurfSense (MODSetter)

| Field | Value |
|---|---|
| Repository | <https://github.com/MODSetter/SurfSense> |
| Version | **v0.0.36** (2026-08-06) — still 0.0.x despite the adoption |
| Stars / activity | ~15,833 · last push 2026-08-08 |
| Language / license | Python · ⚠️ **NOASSERTION** — non-standard license |
| Local-first | ❌ mostly — external connectors and API keys |

## What it does

An open-source NotebookLM alternative: research the open web with live data (Reddit, YouTube,
Instagram, TikTok, Indeed, Google Search, Maps) through one platform, an API, **or an MCP
server**.

## Why it is in this bundle

Two reasons, both about shape rather than technology.

1. **It competes for the same "one server, many sources" mindshare** at ~15.8k stars, while
   being web-first rather than disk-first. When a user asks "what should my agent be able to
   look things up in?", SurfSense is a louder answer than any local retrieval server here.
   Tarn's counter-position is specific and worth stating: **your files, offline, no keys** —
   a corpus SurfSense structurally cannot serve.
2. **The triple-surface pattern.** "One platform, API, or MCP server" is the same shape
   [Pharos](pharos-rag.md) and [engraph](engraph.md) chose, and it is where successful projects
   in this space converge. The important corollary is
   [Pharos](pharos-rag.md)'s: if you ship more than one surface, enforce a **single shared tool
   contract** so they cannot drift.

## Relevance to Tarn

Low. Different corpus, different trust model, different failure modes. Included for
completeness because of its adoption and because "one server, many sources" is the phrase Tarn
is also reaching for.
