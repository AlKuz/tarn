---
type: Competitor Profile
title: mcp-alchemy
description: SQLAlchemy as a universal database adapter — one abstraction, many backends, which is precisely the shape Tarn's extractor layer needs.
resource: https://github.com/runekaagaard/mcp-alchemy
tags: [structured, python, sqlalchemy, adapter-pattern, architecture]
timestamp: 2026-08-09
category: structured
version: unverified
version_checked: 2026-08-09
status: active
depth: profile
---

# mcp-alchemy (Rune Kaagaard)

| Field | Value |
|---|---|
| Repository | <https://github.com/runekaagaard/mcp-alchemy> |
| Version | **no GitHub releases — PyPI-distributed, version unverified** |
| Stars / activity | ~414 · last push 2026-07-31 (active) |
| Language / license | Python · MPL-2.0 |
| Databases | SQLite, Postgres, MySQL/MariaDB, Oracle, MS-SQL — via SQLAlchemy |

## The architectural point

**One adapter layer, N backends.** mcp-alchemy supports five database families not by writing
five integrations but by depending on SQLAlchemy's dialect system and exposing whatever it
supports. The project's own framing is that it offers "knowledge *about*" the database, not
merely access to it.

This is the cleanest small-scale illustration in the survey of the shape Tarn's pivot needs:

```
trait Extractor  →  N format implementations
```

rather than N bespoke pipelines. The equivalents already visible elsewhere in this bundle:

- [xberg](../general/xberg.md) — one crate, 101 formats, one `DocumentStructure` output.
- [lore](../general/lore.md) — one `kreuzberg` dependency behind ten source loaders.
- [Onyx](../general/onyx.md) — one connector abstraction behind 40+ SaaS sources.

**The design consequence for Tarn:** the value is in the *uniform output type*, not the number
of inputs. SQLAlchemy's dialects all produce the same schema objects; xberg's extractors all
produce the same `DocumentStructure`. Tarn's extractor trait should be defined by what it
guarantees — a section stream carrying `heading_path`, provenance, and a byte or page span —
so that adding a format is implementing an interface rather than extending the retrieval layer.

That fits Tarn's existing `Buildable`/`Configurable` pattern directly: an `ExtractorConfig`
dispatch enum wrapping per-format configs, each building a concrete extractor.

## Relevance to Tarn

Low as a competitor. Included because it is the clearest statement of the adapter shape, at a
size small enough to read in one sitting.
