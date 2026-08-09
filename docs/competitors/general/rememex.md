---
type: Competitor Profile
title: Rememex
description: Windows-only Rust local file search over 120+ file types with built-in OCR and EXIF geolocation — proof of demand for exactly Tarn's pivot, left unserved on macOS and Linux.
resource: https://github.com/illegal-instruction-co/rememex
tags: [general, rust, windows-only, ocr, exif, multi-format, stalled]
timestamp: 2026-08-09
category: general
version: 2.5.1
version_checked: 2026-08-09
status: dormant
depth: profile
---

# Rememex (illegal-instruction-co)

| Field | Value |
|---|---|
| Repository | <https://github.com/illegal-instruction-co/rememex> |
| Version | **v2.5.1** (2026-02-19) |
| Stars / activity | ~67 · last push **2026-02-19** — ~6 months quiet |
| Language / license | **Rust** · ⚠️ **no license detected by the GitHub API** (README claims MIT) |
| Platform | ⚠️ **Windows 10+ only** — uses UWP OCR and Mica backdrop |

## What it does

Indexes **120+ file types** (code, documents, images, configs) with **built-in image OCR** and
**EXIF geolocation reverse-geocoded to city names**. Semantic plus keyword hybrid retrieval —
"you type meaning, it finds files". Fully local, with a built-in MCP server.

## Why it matters to Tarn

**It is the closest existing product to the pivot's pitch, and it is unavailable to most of the
market.** "120+ file types, local, Rust, MCP" is precisely Tarn's target description. Rememex
built it, found an audience, and then hard-bound itself to Windows via UWP OCR — leaving
macOS and Linux unserved — and has been quiet for six months.

Three specifics:

- **OCR and EXIF as differentiators.** Scanned documents and photographs are a real part of a
  personal corpus and essentially nobody in this survey indexes them.
  [xberg](xberg.md) supplies both capabilities in Rust (Tesseract / PaddleOCR / Candle / VLM
  with fallback chains), so this is reachable for Tarn without platform lock-in.
- **Its README's competitive comparison table** — versus ripgrep, Everything, Sourcegraph, and
  Recall — is an excellent positioning template. Comparing against the tools users *actually
  have* rather than against boutique competitors is the same instinct as benchmarking against
  [filesystem MCP + grep](mcp-reference-servers.md).
- **The platform choice is the cautionary tale.** Reaching for a platform-native API (UWP OCR)
  to ship OCR quickly cost the project two thirds of its addressable market permanently.

## Licensing caution

The GitHub API detects no license despite a README badge. Treat as all-rights-reserved for any
code reuse — the same situation as
[obsidian-tools](../obsidian-pkm/obsidian-tools-glibalien.md) and
[obsidian-mcp (Storks)](../obsidian-pkm/obsidian-mcp-storks.md).
