---
type: Competitor Profile
title: Vendor database MCP servers (grouped)
description: Every database vendor now ships an MCP server — control plane plus SQL, no retrieval — which makes "SQL over MCP" a commodity Tarn should not compete in head-on.
resource: https://github.com/supabase/mcp
tags: [structured, vendor, commodity, cloud-dependent, duckdb]
timestamp: 2026-08-09
category: structured
version: various
version_checked: 2026-08-09
status: active
depth: profile
---

# Vendor database MCP servers — grouped

All are **cloud or API-key dependent**, all serve a control plane plus SQL execution, and
**none of them do retrieval**.

| Server | Repository | Stars | Latest | Activity | Stack |
|---|---|---|---|---|---|
| Supabase MCP | [supabase/mcp](https://github.com/supabase/mcp) — ⚠️ renamed from `supabase-community/supabase-mcp` | ~2,857 | `mcp-server-supabase-v0.9.0` (2026-07-17) | 2026-08-06 | TS, Apache-2.0 |
| ClickHouse MCP | [ClickHouse/mcp-clickhouse](https://github.com/ClickHouse/mcp-clickhouse) | ~845 | v0.4.1 (2026-07-17) | 2026-08-05 | Python, Apache-2.0 |
| Neon MCP | [neondatabase/mcp-server-neon](https://github.com/neondatabase/mcp-server-neon) — ⚠️ renamed from `neondatabase-labs/` | ~621 | *no releases — unverified* | 2026-08-08 | TS, MIT |
| MotherDuck / DuckDB MCP | [motherduckdb/mcp-server-motherduck](https://github.com/motherduckdb/mcp-server-motherduck) | ~505 | v1.0.7 (2026-06-09) | 2026-07-27 | Python, MIT |
| Snowflake MCP | [Snowflake-Labs/mcp](https://github.com/Snowflake-Labs/mcp) | ~295 | *unverified* | 2026-05-15 (slowing) | Python, Apache-2.0 |

Databricks has no single canonical public `databricks/mcp` repository — **unverified**.

## The conclusion for Tarn

**"SQL over MCP" is commoditized.** Every vendor ships one, they are all free, and they are all
better integrated with their own platform than a third party could be. Tarn should not build a
query-execution server.

What is *not* commoditized is what [XiYan-SQL](xiyan-sql.md) does and nobody else does:
**treating schema elements as ranked retrieval units in the same index as everything else.**
That is the only defensible SQL play for Tarn, and it is a retrieval feature, not a database
feature.

## The exception worth tracking: DuckDB

**`mcp-server-motherduck` runs fully local against DuckDB files.** That makes DuckDB the natural
backend for a *local structured data* source kind — CSV, Parquet, and `.duckdb` files sitting in
a project directory alongside notes and code. Those are files on disk, which is Tarn's corpus,
and they are exactly the kind of thing a mixed personal corpus contains and no current tool
indexes.

That is a materially different and smaller proposition than "connect to my production
Postgres", and it is the one worth considering.
