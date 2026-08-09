---
type: Competitor Profile
title: mcpdoc (LangChain)
description: Fetch-and-list documentation server that trusts llms.txt as the index — evidence that a large slice of the market needs structure plus fetch, with no ranking at all.
resource: https://github.com/langchain-ai/mcpdoc
tags: [documents, python, llms-txt, no-ranking, fetch]
timestamp: 2026-08-09
category: documents
version: 0.0.10
version_checked: 2026-08-09
status: active
depth: profile
---

# mcpdoc (langchain-ai)

| Field | Value |
|---|---|
| Repository | <https://github.com/langchain-ai/mcpdoc> |
| Version | **mcpdoc 0.0.10** (2025-07-22) |
| Stars / activity | ~1,024 · last push 2026-07-17 |
| Language / license | Python · MIT |

## What it does

Exposes **llms.txt-indexed** documentation sources to IDEs and agents, with domain
allow-listing for safety. No embeddings, no BM25, **no ranking of any kind** — it is a
fetch-and-list server that trusts `llms.txt` as a pre-authored index.

## Why it matters

**A thousand stars for a server with no retrieval.** That is the useful signal: for a
well-curated documentation set, *structure plus fetch* is sufficient, and ranking is
unnecessary. The author of the docs has already done the retrieval work by writing the index.

This is the same argument [Cube](../structured/cube-semantic-layer.md) makes for SQL — curate a
small meaningful surface instead of ranking a large one — and it is worth taking seriously as
the boundary of Tarn's value. Tarn earns its index where curation is absent or impossible:
personal vaults, mixed corpora, large source trees, PDF archives. Where a good `llms.txt`
exists, ranking adds little.

## The cheap win

**Ingest `llms.txt` / `llms-full.txt` as a source kind.** For web-scraped documentation, the
file is a curated map of what matters — it tells Tarn which pages to fetch and how the author
groups them. That is better metadata than a crawler can infer, it is free, and it slots
directly into the `[[sources]]` model as a `kind = "llms_txt"` alongside
[lore](../general/lore.md)'s sitemap and feed loaders.

Domain allow-listing, as mcpdoc implements it, should come with it — any URL-fetching source
needs that guard.
