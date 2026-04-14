# local-index

## What This Is

A Rust daemon that watches a directory tree (initially an Obsidian vault), chunks markdown files by heading, embeds each chunk via the Anthropic API, and stores everything in an embedded LanceDB database. Exposes full-text and semantic search via CLI, a Claude Code skill interface, and a web dashboard — enabling Claude to reason over your notes without resorting to grep.

## Core Value

Fast, accurate semantic search over a local markdown vault that Claude can query as a skill without any manual intervention.

## Current Milestone: v1.1 — Search UX & Observability

**Goal:** Make search results more useful and operational logs actually readable.

**Target features:**
- Web UI: reranking toggle (expose Claude reranking in search UI)
- Web UI: highlight matching words/phrases in result snippets
- Logging: log search queries with mode, result count, and latency
- Logging: log daemon file events (creates, modifies, renames, deletes) with indexing outcomes
- Logging: suppress noisy LanceDB internal messages (full source paths)

---

## Requirements

### Validated

**Phase 6: Claude Code Integration** (validated 2026-04-13)

- [x] Claude Code skill files for search, reindex, and status (`.claude/skills/`)
- [x] Documented shell wrapper scripts for search, reindex, and status
- [x] README section documenting Claude Code integration

**Phase 5: Web Dashboard** (validated 2026-04-12)

- [x] axum HTTP server on configurable port (default 3000), binds 127.0.0.1, `--bind` flag
- [x] Search UI: text input, mode selector, results with chunk text, file path, breadcrumb, score
- [x] Index browser: all indexed files with per-file chunk count and last-indexed timestamp
- [x] Index status view: total chunks/files, last full-index time, queue depth, stale count
- [x] Embedding stats view: total embeddings, model ID, estimated token usage
- [x] Read-only settings view: current config values, credential source, active flags

**Phase 4: Daemon Mode & Observability** (validated 2026-04-10)

- [x] `local-index daemon <path>` watches for file create/modify/rename/delete events
- [x] File rename → delete old path + index new path; file delete → remove all chunks for that file
- [x] File watcher, embedding pipeline, and HTTP server run concurrently in single tokio runtime
- [x] Graceful shutdown via broadcast channel on SIGINT/SIGTERM
- [x] `local-index status` shows total chunks/files, last index time, queue depth, stale count
- [x] `/metrics` Prometheus endpoint with HDR histograms for embedding, indexing, search, HTTP latency
- [x] Counter metrics (chunks indexed, API errors, file events, search queries) and gauge metrics (queue depth, chunk/file totals)

**Phase 3: Search** (validated 2026-04-10)

- [x] SearchEngine dispatching semantic (cosine), FTS (BM25), and hybrid (RRF k=60) queries through LanceDB
- [x] Score normalization: semantic = 1-(dist/2), FTS = score/max, hybrid = relevance/max
- [x] Path prefix and tag post-filters; min_score threshold; context chunk assembly
- [x] JSON formatter (wrapped object: query/mode/total/results) and pretty snippet formatter (═ separator, 200-char truncate)
- [x] `local-index search` CLI wired with --limit, --min-score, --mode, --path-filter, --tag-filter, --context, --format
- [x] FTS index created eagerly during `index` command; ensure_fts_index() lazy fallback on first search
- [x] Integration test suite (9 tests) with MockEmbedder against real LanceDB in tempdirs

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

### Active (v1.1)

**Web UI**

- [ ] Search UI exposes a "Rerank results" checkbox; when checked, backend is called with `rerank=true`; results show a "(reranked)" indicator; checkbox disabled when `ANTHROPIC_API_KEY` is absent
- [ ] Search result snippets highlight all query terms (case-insensitive, word boundary) using `<mark>` elements in the displayed chunk_text

**Logging**

- [ ] Every search query logged at INFO with fields: `query`, `mode`, `results_returned`, `latency_ms`
- [ ] Daemon file-watcher events logged at INFO with fields: event type, path, renamed_to (renames); indexing outcome (chunks added/removed/skipped) logged after each event
- [ ] LanceDB internal tracing suppressed below WARN via EnvFilter (`lancedb=warn,lance=warn`), removing verbose source-path noise without losing actionable warnings

### Out of Scope

- PDF support — deferred; requires different extraction pipeline (see SEED-001)
- Remote/cloud LanceDB — embedded only; keeps deployment simple
- Authentication on the WebUI — local tool, no auth needed
- Settings UI in the WebUI — all config via CLI/.env; read-only view is sufficient
- Multi-vault support — single directory root per daemon instance

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
| Both daemon + one-shot modes        | Daemon for real-time watching; one-shot for CI/manual re-index        | Validated Phase 4 — notify + debouncer, tokio runtime, graceful shutdown |
| Prometheus + HDR histograms         | Standard observability; histograms capture tail latency for API calls | Validated Phase 4 — /metrics endpoint, HDR histograms, 4 counter + 4 gauge metrics |
| Claude Code skills + shell wrappers | Skills for Claude integration; shell wrappers for humans and scripts  | Validated Phase 6 — search/reindex/status skills + wrappers in repo |

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
*Last updated: 2026-04-14 — v1.0 complete (all 6 phases), v1.1 milestone started*
