---
type: Competitor Profile
title: Khoj
description: AGPL self-hostable "AI second brain" application indexing local docs and the web — the app end of the spectrum, and the shape Tarn should avoid becoming.
resource: https://github.com/khoj-ai/khoj
tags: [general, python, agpl, application, second-brain, boundary]
timestamp: 2026-08-09
category: general
version: 2.0.0-beta.28
version_checked: 2026-08-09
status: active
depth: profile
---

# Khoj (khoj-ai)

| Field | Value |
|---|---|
| Repository | <https://github.com/khoj-ai/khoj> |
| Version | **2.0.0-beta.28** (2026-03-26) — ⚠️ shipping from `main`; no stable release in ~5 months |
| Stars / activity | ~36,401 · last push 2026-08-02 |
| Language / license | Python · **AGPL-3.0** |

## What it does

A self-hostable "AI second brain": indexes local documents (markdown, PDF, org-mode, Notion,
images) and the web, with custom agents, scheduled automations, and deep research. Works with
local or hosted models. Ships a web UI plus Obsidian and Emacs plugins and a Docker stack.

## Why it is in this bundle

**As a boundary marker.** Khoj is what Tarn becomes if it keeps adding surface: an
application, with a UI, an agent runtime, automations, and a deployment story. At 36k stars
that is clearly a viable product — but it is a different product, competing for the *user's
attention* rather than for a slot in the agent's toolchain.

The useful takeaway is inverted: **Tarn's advantage is being one binary an agent talks to.**
Every UI, scheduler, or agent-runtime feature trades that away. Khoj and
[Onyx](onyx.md) both demonstrate that the platform path works and that it costs you the
single-binary property permanently.

Two smaller notes:

- **Its format list is a reasonable minimum viable extractor set** for the pivot: markdown,
  PDF, org-mode, images, plus one hosted source.
- **AGPL again.** Khoj and [basic-memory](basic-memory.md) are both AGPL; a permissive
  MIT/Apache Rust binary remains a real differentiator for commercial adoption.

## Version caution

No stable release since 2026-03-26 while development continues on `main`. For a self-hosted
application that is a meaningful operational signal — users are running unreleased code.
