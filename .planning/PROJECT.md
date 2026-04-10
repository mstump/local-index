# local-index

## What This Is

A Rust daemon that watches a directory tree (initially an Obsidian vault), chunks markdown files by heading, embeds each chunk via the Anthropic API, and stores everything in an embedded LanceDB database. Exposes full-text and semantic search via CLI, a Claude Code skill interface, and a web dashboard — enabling Claude to reason over your notes without resorting to grep.

## Core Value

Fast, accurate semantic search over a local markdown vault that Claude can query as a skill without any manual intervention.

## Requirements

### Validated

**Phase 2: Storage & Embedding Pipeline** (validated 2026-04-10)

- [x] Chunk markdown files with smart size-based splitting and semantic break-point detection (CHUNK_SIZE_CHARS=3600, 15% overlap)
- [x] Embed chunks via Voyage AI embeddings API (voyage-3.5, 1024 dims, 50/batch, 5x retry with jitter)
- [x] Store chunks + embeddings in embedded LanceDB with 10-column Arrow schema
- [x] Incremental updates: SHA-256 content hash comparison skips unchanged chunks on re-index
- [x] Credential resolution: VOYAGE_API_KEY env var with actionable error on missing
- [x] Model mismatch guard: blocks re-indexing with wrong model unless --force-reindex
- [x] TTY-aware progress: indicatif bar in TTY, per-file eprintln! to stderr in non-TTY, JSON summary to stdout

**Phase 1: Foundation & File Processing** (validated 2026-04-10)

- [x] CLI skeleton with clap + derive macros (index, search, daemon, status, serve subcommands)
- [x] Recursive markdown walker with per-file graceful error handling
- [x] YAML frontmatter parsing with heading breadcrumb extraction
- [x] `tracing` structured logging with RUST_LOG and --log-level support

### Active

**Indexing**

- [ ] Watch a directory tree recursively for file create/modify/move/delete events
- [ ] Chunk markdown files by heading (each heading section = one embedding unit)
- [ ] Embed chunks via Anthropic embeddings API
- [ ] Store chunks + embeddings in embedded LanceDB database
- [ ] Support one-shot indexing mode (`--index`) and persistent daemon mode (`--daemon`)
- [ ] Incremental updates: only re-embed chunks that changed, skip unchanged

**Search**

- [ ] Full-text search over chunk content
- [ ] Semantic (vector) search over embeddings
- [ ] Structured JSON results: chunk text, file path (vault-relative), similarity score, surrounding context lines
- [ ] CLI command: `local-index search "<query>"` with configurable result count and score threshold

**Credentials & Config**

- [ ] Credential resolution: `ANTHROPIC_API_KEY` env var first, fall back to `~/.claude/` credential store
- [ ] All settings via CLI flags (clap + derive), `.env` file, and environment variables — no config file UI

**Observability**

- [ ] Prometheus metrics endpoint (`/metrics`) on the HTTP server
- [ ] HDR histograms (or equivalent) for all latency-sensitive operations: Anthropic API calls, file indexing, search queries, WebUI requests
- [ ] `tracing` crate for structured logging throughout

**WebUI**

- [ ] Search UI: query input, ranked results with snippets and file paths
- [ ] Index browser: list indexed files, per-file chunk count, last-indexed timestamp
- [ ] Index status: total chunks, total files, last full-index time, pending queue depth
- [ ] Embedding stats: embedding count, model used, estimated token usage
- [ ] Read-only settings view (current config, credential source, active flags)
- [ ] Served on configurable port (default: 3000) by the same process

**Skills / Claude Integration**

- [ ] Claude Code skill files (`.claude/skills/`) for: search, re-index, status
- [ ] Documented shell wrapper scripts for the same operations

**CLI**

- [ ] `clap` with derive macros for all commands and flags
- [ ] Subcommands: `index`, `daemon`, `search`, `status`, `serve`

### Out of Scope

- PDF support — deferred to v2; requires different extraction pipeline
- Remote/cloud LanceDB — embedded only for v1; keeps deployment simple
- Authentication on the WebUI — local tool, no auth needed for v1
- Settings UI in the WebUI — all config via CLI/.env; read-only view is sufficient
- Multi-vault support — single directory root per daemon instance in v1

## Context

- **Obsidian vault** is the primary data source: markdown files with YAML frontmatter, wiki-links (`[[note]]`), and nested heading structure
- **LanceDB** is embedded (no separate server), Rust-native via the `lancedb` crate
- **Claude Code credential store** lives at `~/.claude/` — the daemon should parse the same credential format Claude Code uses
- **Anthropic embeddings API** (`text-embedding-3-*` or equivalent) is the embedding source
- **Primary consumer is Claude Code** via skill invocation — output must be machine-parseable JSON
- The web dashboard is a secondary consumer for human browsing and debugging

## Constraints

- **Tech stack**: Rust only — no Node/Python helpers
- **Embeddings**: Anthropic API only in v1 (no local models, no OpenAI)
- **Database**: LanceDB embedded — no external database process
- **CLI framework**: `clap` with derive macros
- **Logging**: `tracing` crate (no `log` crate directly)
- **Metrics**: Prometheus-compatible `/metrics` endpoint; HDR histograms for latency
- **Deployment**: Single binary, runs on macOS (primary), Linux (secondary)

## Key Decisions

| Decision                            | Rationale                                                             | Outcome   |
|-------------------------------------|-----------------------------------------------------------------------|-----------|
| Chunk by heading                    | More precise search results; Obsidian notes are heading-structured    | Validated Phase 1 — smart size-based chunking with heading breadcrumbs |
| Voyage AI for embeddings            | voyage-3.5 chosen over Anthropic; 1024 dims, better retrieval quality | Validated Phase 2 — VOYAGE_API_KEY env var, 50/batch, 5x retry |
| Env var preferred for credentials   | Explicit over implicit; simpler than ~/.claude/ JSON parsing           | Validated Phase 2 — VOYAGE_API_KEY only, clear error on missing |
| Single binary, embedded LanceDB     | Zero deployment friction; no separate database process                | Validated Phase 2 — 10-col Arrow schema, hash-based incremental indexing |
| Both daemon + one-shot modes        | Daemon for real-time watching; one-shot for CI/manual re-index        | — Pending (Phase 4+) |
| Prometheus + HDR histograms         | Standard observability; histograms capture tail latency for API calls | — Pending (Phase 4) |
| Claude Code skills + shell wrappers | Skills for Claude integration; shell wrappers for humans and scripts  | — Pending (Phase 6) |

## Evolution

This document evolves at phase transitions and milestone boundaries.

**After each phase transition** (via `/gsd:transition`):

1. Requirements invalidated? → Move to Out of Scope with reason
2. Requirements validated? → Move to Validated with phase reference
3. New requirements emerged? → Add to Active
4. Decisions to log? → Add to Key Decisions
5. "What This Is" still accurate? → Update if drifted

**After each milestone** (via `/gsd:complete-milestone`):

1. Full review of all sections
2. Core Value check — still the right priority?
3. Audit Out of Scope — reasons still valid?
4. Update Context with current state

---
*Last updated: 2026-04-10 after Phase 2 completion — storage & embedding pipeline validated*
