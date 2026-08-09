---
type: Competitor Profile
title: obsidian-mcp (eddmann)
description: Git-backed MCP server for Obsidian vaults with semantic anchors, unified-diff patching, and AWS Lambda deployment.
resource: https://github.com/eddmann/obsidian-mcp
tags: [obsidian-pkm, typescript, git-backed, lambda, patching, dormant]
timestamp: 2026-08-09
category: obsidian-pkm
version: 1.0.0
version_checked: 2026-08-09
status: dormant
depth: deep-dive
---

# Competitor Analysis: obsidian-mcp (eddmann)

## Overview

| Field | Value |
|-------|-------|
| **Repository** | https://github.com/eddmann/obsidian-mcp |
| **Language** | TypeScript |
| **Version** | 1.0.0 |
| **License** | MIT |
| **Stars** | ~13 |
| **Last activity** | 2025-12-03 |
| **Status** | Dormant |
| **Runtime** | Node.js >= 22 |
| **Architecture** | Git-backed MCP server (does NOT connect to Obsidian directly) |

### Maturity Assessment

The project reached **early production** maturity and then stopped. It has a clean monorepo structure, comprehensive OAuth 2.0 support, three deployment modes (stdio, HTTP, Lambda), Docker packaging, CDK infrastructure-as-code, and a test suite using Vitest. The codebase is well-organized with proper separation of concerns. The CLAUDE.md file is extremely thorough (approximately 400 lines), suggesting active AI-assisted development. The `1.0.0` version indicates the author considered it release-ready. GitHub releases use Docker image publishing via GHCR.

> **Dormant since 2025-12-03** — eight months without a commit, still at the 1.0.0 it shipped
> with. The ideas below remain the most interesting part of this project: its semantic
> anchors and unified-diff patching are still the best write-targeting design in the
> Obsidian/PKM category, and nobody has picked them up.

---

## Technology Stack

### Runtime & Frameworks

| Component | Technology |
|-----------|-----------|
| Runtime | Node.js 22+ |
| Language | TypeScript 5.7+ |
| MCP SDK | `@modelcontextprotocol/sdk` ^1.0.4 |
| HTTP Server | Express 4.x |
| Git Operations | `simple-git` ^3.27.0 |
| Search | `fuse.js` ^7.0.0 (fuzzy search) |
| Diff Patching | `diff` ^8.0.0 |
| Schema Validation | `zod` ^3.24.1 |
| Serverless | `serverless-http` ^3.2.0 |
| AWS DynamoDB | `@aws-sdk/client-dynamodb` ^3.540.0 |
| Environment | `dotenv` ^16.4.5 |

### Build Tooling

| Tool | Purpose |
|------|---------|
| TypeScript (`tsc`) | Type checking |
| esbuild | Bundling for stdio, HTTP, and Lambda targets |
| Vitest 4.x | Testing |
| ESLint + Prettier | Linting and formatting |
| AWS CDK 2.x | Infrastructure deployment |
| Docker | Container packaging (multi-stage build) |

### Monorepo Structure

```
packages/
  app/          # Core MCP server (all handlers, services, transports)
  cdk/          # AWS CDK infrastructure stack
```

Uses npm workspaces. The `app` package produces three separate esbuild bundles: `dist/stdio/index.js`, `dist/http/index.js`, and `dist/lambda/index.js`.

---

## Architecture

### Connection Model

This server does **not** connect to a running Obsidian instance. Instead, it operates on a **git-backed clone** of the vault:

1. On startup (or each request), the server clones/pulls the vault from a remote git repository
2. LLM tools read and modify files in the local clone
3. Every write operation automatically commits and pushes changes
4. Obsidian clients sync via the `obsidian-git` plugin

This is fundamentally different from approaches that use Obsidian's REST API or local file access. It enables **headless operation** without Obsidian running.

### Component Diagram

```mermaid
graph TB
    subgraph "MCP Clients"
        CD[Claude Desktop]
        CW[ChatGPT / Claude Web]
    end

    subgraph "Transport Layer"
        STDIO[StdioServerTransport]
        HTTP[StreamableHTTPServerTransport]
    end

    subgraph "Auth Layer (HTTP/Lambda only)"
        OAUTH[OAuth 2.0 Routes<br/>Authorization Code + PKCE]
        SM[Session Manager]
        IMS[In-Memory Store]
        DDB[DynamoDB Store]
    end

    subgraph "MCP Server Core"
        MCP[McpServer<br/>@modelcontextprotocol/sdk]
        TR[Tool Registrations<br/>18 tools]
        RR[Resource Registrations<br/>1 resource]
    end

    subgraph "Tool Handlers"
        FH[File Handlers<br/>9 tools]
        DH[Directory Handlers<br/>3 tools]
        SH[Search Handlers<br/>1 tool]
        TH[Tag Handlers<br/>4 tools]
        JH[Journal Handler<br/>1 tool]
    end

    subgraph "Services"
        VM[VaultManager<br/>interface]
        GVM[GitVaultManager<br/>simple-git]
        GAP[Git Auth Provider<br/>GitHub/GitLab/Bitbucket/Generic]
        JF[Journal Formatter]
    end

    subgraph "Storage"
        GIT[(Remote Git Repository)]
        LOCAL[Local Vault Clone<br/>filesystem]
    end

    CD --> STDIO
    CW --> HTTP

    HTTP --> OAUTH
    OAUTH --> SM
    SM --> IMS
    SM --> DDB

    STDIO --> MCP
    HTTP --> MCP

    MCP --> TR
    MCP --> RR

    TR --> FH
    TR --> DH
    TR --> SH
    TR --> TH
    TR --> JH

    FH --> VM
    DH --> VM
    SH --> VM
    TH --> VM
    JH --> VM
    JH --> JF

    VM --> GVM
    GVM --> GAP
    GVM --> LOCAL
    GVM --> GIT
```

### Deployment Modes

| Mode | Transport | Auth | Session Storage | Use Case |
|------|-----------|------|-----------------|----------|
| **Stdio** | StdioServerTransport | None | N/A | Claude Desktop, Cursor |
| **HTTP** | StreamableHTTPServerTransport | OAuth 2.0 + PKCE | In-memory | ChatGPT, remote clients |
| **Lambda** | serverless-http + Express | OAuth 2.0 + PKCE | DynamoDB | Serverless production |

### Git Synchronization Model

Every `VaultManager` method calls `initialize()` first, which:
- **Cold start**: Clones the repo with `--depth 1 --single-branch`
- **Warm start**: Fetches and does `git reset --hard origin/<branch>` + `git clean -fdx`

Every write operation (`writeFile`, `deleteFile`, `moveFile`) automatically calls `commitAndPush()`:
1. Stages affected files with `git add -A`
2. Commits with descriptive message (e.g., `"Update file: path/to/note.md"`)
3. Pushes with exponential backoff retry (up to 3 attempts)

**Critical implication**: Every tool invocation that writes triggers a full git commit and push cycle. This means individual tool calls are expensive in terms of I/O and network latency.

---

## Tools & Capabilities

### File Operations (9 tools)

| Tool | Parameters | Description |
|------|-----------|-------------|
| `read-note` | `path: string` | Read a single note's contents |
| `read-notes` | `paths: string[]` (max 50) | Batch read multiple notes. Returns partial success results |
| `create-note` | `path`, `content`, `overwrite?` | Create a new note. Fails if exists unless `overwrite=true` |
| `edit-note` | `path`, `content` | Full content replacement of an existing note |
| `delete-note` | `path`, `confirm: boolean` | Delete a note. Requires explicit `confirm=true` |
| `move-note` | `source_path`, `destination_path`, `overwrite?` | Move/rename a note |
| `append-content` | `path`, `content`, `newline?`, `create_if_missing?` | Append to end of file. Can auto-create |
| `patch-content` | `path`, `content`, `anchor_type`, `anchor_value`, `position`, `create_if_missing?` | Insert content at specific anchors: `heading`, `block`, `frontmatter`, `text_match`. Positions: `before`, `after`, `replace` |
| `apply-diff-patch` | `path`, `diff` | Apply unified diff patch. Uses the `diff` npm package (`applyPatch`). Strict matching, no fuzz |

### Directory Operations (3 tools)

| Tool | Parameters | Description |
|------|-----------|-------------|
| `create-directory` | `path`, `recursive?` | Create directory. Also creates a `.gitkeep` file for git tracking |
| `list-files-in-vault` | `include_directories?`, `file_types?`, `recursive?` | List files from vault root |
| `list-files-in-dir` | `path`, `include_directories?`, `file_types?`, `recursive?` | List files in a specific directory |

### Search (1 tool)

| Tool | Parameters | Description |
|------|-----------|-------------|
| `search-vault` | `query`, `exact?`, `path_filter?`, `file_types?`, `limit?` | Fuzzy (fuse.js) or exact substring search across filenames and content. Returns relevance scores 1-4 with context lines |

### Tag Management (4 tools)

| Tool | Parameters | Description |
|------|-----------|-------------|
| `add-tags` | `path`, `tags[]`, `location?`, `deduplicate?` | Add tags to frontmatter, inline, or both |
| `remove-tags` | `path`, `tags[]`, `location?` | Remove tags from frontmatter and/or inline |
| `rename-tag` | `old_tag`, `new_tag`, `case_sensitive?`, `dry_run?` | Rename a tag across ALL vault files |
| `manage-tags` | `action`, `tag?`, `merge_into?`, `sort_by?`, `include_nested?` | List all tags with counts, or merge tags |

### Journal Logging (1 tool)

| Tool | Parameters | Description |
|------|-----------|-------------|
| `log-journal-entry` | `activity_type`, `summary`, `key_topics[]`, `outputs?`, `project?` | Log timestamped activity to daily journal. Activity types: development, research, writing, planning, learning, problem-solving. Auto-creates journal from template |

### Resources (1)

| Resource | URI | Description |
|----------|-----|-------------|
| `vault-readme` | `obsidian://vault-readme` | Serves `README.md` from vault root for vault organization context |

### Tool Annotations

All tools use MCP tool annotations:
- `readOnlyHint` -- correctly marks read vs. write operations
- `destructiveHint` -- marks `edit-note`, `delete-note`, `move-note`, `apply-diff-patch` as destructive
- `idempotentHint` -- correctly identifies idempotent operations
- `openWorldHint: true` -- all tools interact with external git-backed vault

---

## Search Implementation

### Algorithm: Fuse.js Fuzzy Search

The search is implemented in `packages/app/src/mcp/handlers/search-handlers.ts` with two modes:

#### Fuzzy Search (default)

1. **Filename search**: Creates a `Fuse` instance over filenames with `threshold: 0.4`, `ignoreLocation: true`. Filters results with `score > 0.6` as poor quality
2. **Content search**: Reads ALL files into memory (batches of 10 concurrent reads), creates a second `Fuse` instance over full file content strings
3. **Line-level matching**: After fuse.js identifies matching files, a secondary `fuzzyMatchLine()` function tokenizes the query into words and uses a NEW `Fuse` instance per line per token to check if all query tokens fuzzy-match at least one word in the line

#### Exact Search

Simple case-insensitive `string.includes()` across filenames and file content line by line.

#### Relevance Scoring

Returns scores 1-4 (lower is better):
- Fuzzy: Maps fuse.js scores (0-1) to 4 buckets: `<0.25` = 1 (excellent), `<0.5` = 2 (good), `<0.75` = 3 (fair), rest = 4 (poor)
- Exact: Checks word boundaries, line-start matches, and position within line

#### Context

Returns 2 lines before and 2 lines after each matching line.

### Performance Characteristics

- **No persistent index**: Every search reads ALL vault files from disk
- **Full file content in memory**: Entire vault content is loaded into memory for content search
- **O(n) file reads**: Reads every file in the vault for every search query
- **Per-line Fuse instantiation**: In fuzzy mode, `fuzzyMatchLine()` creates a NEW `Fuse` instance for every line for every query token -- this is extremely inefficient
- **Batch size 10**: Concurrent file reads capped at 10 at a time
- **Default limit 50**: Results capped at 50 by default
- **No caching between searches**: No search index persistence (the Lambda `cache.ts` file index is unused by search)

### Scaling Assessment

This search approach works acceptably for small vaults (hundreds of notes) but will degrade significantly for large vaults (thousands+ notes) due to full-scan reads and per-line Fuse instantiation overhead.

---

## Token & Cost Optimization

### Response Format

- Tool responses are JSON-serialized with `JSON.stringify(data, null, 2)` (pretty-printed)
- Successful responses include both `content[0].text` (JSON string) and `structuredContent` (parsed object)
- Error responses return plain text error messages

### Context Management

- **No section-level granularity**: `read-note` returns full file content. There is no ability to read specific sections of a note
- **No content summarization**: No truncation, compression, or summarization of returned content
- **Batch reads**: `read-notes` supports reading up to 50 files in a single call, reducing round trips but not token usage
- **Search context lines**: Search returns only 2 lines of context before/after matches rather than full file content
- **Change previews**: `patch-content` and `apply-diff-patch` return a `change_preview` with the affected line range and surrounding context instead of the full updated file
- **Result limits**: Search defaults to 50 results max

### Server Instructions

The server provides `MCP_SERVER_INSTRUCTIONS` to LLM clients, instructing them to proactively use the `log-journal-entry` tool after completing tasks. This adds overhead per conversation but provides value to the user.

### Assessment

Token optimization is minimal. Full file contents are returned with no section-level retrieval, no content windowing, and no response compression. For large notes (thousands of lines), this will consume significant context window budget.

---

## Security Model

### Authentication

| Mode | Mechanism |
|------|-----------|
| Stdio | None (local trust) |
| HTTP | OAuth 2.0 Authorization Code Flow with PKCE |
| Lambda | OAuth 2.0 + DynamoDB sessions |

### OAuth Implementation Details

- Full OAuth 2.0 spec compliance: `.well-known/oauth-authorization-server` discovery, authorization, token, registration, revocation endpoints
- PKCE support: Both `S256` and `plain` challenge methods
- Session management: 24-hour TTL (configurable via `SESSION_EXPIRY_MS`)
- Token comparison: Uses `crypto.timingSafeEqual` for constant-time comparison (prevents timing attacks)
- Session IDs: Generated with `crypto.randomBytes(32)` (256-bit entropy)
- Cookies: `httpOnly`, `secure` (when HTTPS), `sameSite: lax`

### Path Restrictions

**There are no path traversal protections.** The `VaultManager` interface accepts raw relative paths and joins them directly with `path.join(this.config.vaultPath, relativePath)`. There is no validation that:
- The resolved path stays within the vault root
- The path does not contain `..` segments
- The path does not reference `.git` or `.obsidian` directories (except in `listFiles` which skips them during enumeration)

An LLM could potentially read or write to `../../etc/passwd` relative to the vault root. This is a significant security gap.

### Git Credential Handling

- Credentials are embedded in the git remote URL (e.g., `https://x-access-token:TOKEN@github.com/...`)
- The `sanitizeUrl()` method masks credentials when logging
- `GIT_TERMINAL_PROMPT=0` prevents interactive credential prompts
- Git provider auto-detection supports GitHub, GitLab, Bitbucket, and generic providers

### Input Validation

- Zod schemas validate tool input types and shapes
- `delete-note` requires explicit `confirm: true` as a guard
- `create-note` prevents overwrites by default (requires `overwrite: true`)
- `patch-content` with `text_match` detects ambiguous multi-match situations and fails with a helpful error
- No file size limits on reads or writes
- No rate limiting on tool invocations

---

## Strengths & Weaknesses

### Strengths

1. **Comprehensive write tooling**: 9 file operation tools covering create, read, edit, delete, move, append, patch (semantic anchors), and unified diff patching. The `patch-content` tool with heading/block/frontmatter/text_match anchors is particularly well-designed for LLM use

2. **Three deployment modes**: Stdio for local development, HTTP with OAuth for remote clients, Lambda for serverless. This covers all major deployment scenarios

3. **Full OAuth 2.0 implementation**: Proper Authorization Code Flow with PKCE, discovery endpoints, refresh tokens, session management, and timing-safe token comparison. Production-grade auth

4. **Git-backed synchronization**: Enables headless operation without Obsidian running. Auto-commit and push after every write. Exponential backoff retry on push failures

5. **Tag management**: Dedicated tools for adding, removing, renaming, and managing tags across the entire vault. Supports both frontmatter and inline tags

6. **Journal logging**: Unique feature that auto-logs LLM activity to daily journal files with structured entries (timestamps, activity types, topic tags, project links). Template-based journal creation

7. **Batch operations**: `read-notes` supports reading up to 50 files with partial success handling

8. **Tool annotations**: Proper use of MCP tool annotations for `readOnlyHint`, `destructiveHint`, `idempotentHint`

9. **AWS CDK infrastructure**: Complete infrastructure-as-code for Lambda + DynamoDB deployment

### Weaknesses

1. **No path traversal protection**: Raw path joining without validation. An LLM could read/write files outside the vault root. This is a critical security vulnerability

2. **Extremely inefficient search**: Full-scan reads every file for every query. Per-line Fuse.js instantiation in fuzzy mode creates potentially thousands of Fuse instances per search. No persistent index

3. **No section-level retrieval**: `read-note` returns entire file content. No ability to read specific headings or sections. For large notes, this wastes significant context window tokens

4. **Git sync on every operation**: `initialize()` is called at the start of every `VaultManager` method (read, write, list, exists). For warm starts this means a `fetch + reset --hard + clean` on every single tool call. This adds significant latency

5. **No Obsidian syntax awareness**: No markdown parsing, no wikilink resolution, no backlink tracking, no graph queries. Files are treated as plain text blobs (except for tag regex matching)

6. **No concurrency control**: No optimistic locking or revision tokens. Two concurrent LLM sessions could overwrite each other's changes. The `git reset --hard` sync strategy discards any local uncommitted changes

7. **No content size limits**: No protection against reading or writing extremely large files that could consume the LLM's entire context window

8. **Full vault in memory for search**: Content search loads entire vault into memory. For large vaults (10k+ notes) this could cause memory pressure, especially in Lambda (2GB limit)

9. **Hardcoded git user**: Commits always use `Obsidian MCP Server <mcp@obsidian.local>`. No way to attribute changes to specific users in multi-user scenarios

10. **Journal feature requires configuration**: 4 environment variables (`JOURNAL_PATH_TEMPLATE`, `JOURNAL_DATE_FORMAT`, `JOURNAL_ACTIVITY_SECTION`, `JOURNAL_FILE_TEMPLATE`) are required even if journal logging is not used, as they are listed in `CORE_ENV_VARS`

## Comparison with Tarn

| Aspect | obsidian-mcp (eddmann) | Tarn |
|--------|----------------------|------|
| Language | TypeScript / Node.js | Rust |
| Vault access | Git clone (headless) | Direct local filesystem |
| Search | Fuse.js full-scan | BM25 with persistent index |
| Index unit | None (whole files) | Sections (heading-delimited) |
| Path safety | None | `VaultPath` validated type |
| Concurrency | None | `RevisionToken` optimistic locking |
| Markdown parsing | Regex only | Full parser (wikilinks, frontmatter, embeds) |
| Write tools | 9 (comprehensive) | In development |
| Deployment | stdio / HTTP / Lambda | stdio |
| Auth | OAuth 2.0 + PKCE | None |
| Tag management | 4 dedicated tools | Via note parsing |
