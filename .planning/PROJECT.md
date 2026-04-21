# local-index

## What This Is

A Rust daemon that watches a directory tree (initially an Obsidian vault), chunks markdown files by heading, embeds each chunk via the Voyage AI API, and stores everything in an embedded LanceDB database. A companion asset pipeline preprocesses PDFs and images via Anthropic vision/OCR and writes their derived text directly into LanceDB as indexed chunks. Exposes full-text and semantic search via CLI, a Claude Code skill interface, and a web dashboard — enabling Claude to reason over your notes without resorting to grep.

## Core Value

Fast, accurate semantic search over a local markdown vault that Claude can query as a skill without any manual intervention.

## Previous milestone: v1.2 — PDF & Image Preprocessor (complete 2026-04-20)

**Shipped in v1.2:**
- PDF classification (TextFirst vs NeedsVision) + local text extraction via lopdf — Phase 9
- Anthropic OCR for scanned PDFs (NeedsVision path); optional Google Document AI OCR — Phase 9/10
- Standalone image vision via Anthropic (PNG/JPG/WEBP) — Phase 9
- `OcrService` enum dispatch; `--ocr-provider` / `LOCAL_INDEX_OCR_PROVIDER` — Phase 10
- Google Document AI with JWT service-account auth — Phase 10
- Ephemeral SHA-256 asset cache (`asset-cache/{shard}/{sha256}.txt`) for idempotent re-indexing (PRE-04) — Phase 11
- Canonical blockquote image format `> **[Image: {filename}]** {desc}` for all vision output (PRE-11/PRE-12) — Phase 11
- TextFirst PDF per-page text + embedded-image vision interleaving via pdfium-render (PRE-09/PRE-10) — Phase 11
- Graceful degradation: missing pdfium or ANTHROPIC_API_KEY → text-only with WARN — Phase 11
- README documentation of ephemeral-cache approach, double-index prevention, cache invalidation (PRE-13) — Phase 11

**Selected seed:** [SEED-001](.planning/seeds/SEED-001-pdf-image-processor-daemon.md)

---

## Previous milestone: v1.1 — Search UX & Observability (complete 2026-04-14)

**Shipped in v1.1:**
- Web UI: reranking toggle (Claude reranking exposed in search UI) — Phase 8
- Web UI: highlight matching words/phrases in result snippets — Phase 8
- Logging: search queries with mode, result count, and latency — Phase 7
- Logging: daemon file events (creates, modifies, renames, deletes) with indexing outcomes — Phase 7
- Logging: suppress noisy LanceDB internal messages (full source paths) — Phase 7

---

## Requirements

### Validated

**Phase 11: Vision enrichment & idempotency** (validated 2026-04-20)

- ✓ SHA-256 cache-read gate at top of `ingest_asset_path`; unchanged sources skip all OCR/vision API calls — v1.2
- ✓ Canonical blockquote format `> **[Image: {filename}]** {desc}` for standalone images and PDF pages — v1.2
- ✓ TextFirst PDF per-page text + embedded-image vision interleaving; pages joined by `---` — v1.2
- ✓ Graceful degradation at 3 levels: no pdfium, no `ANTHROPIC_API_KEY`, per-image vision failure — v1.2
- ✓ README documents ephemeral cache layout, invalidation procedure, double-index prevention — v1.2

**Phase 10: OCR providers** (validated 2026-04-20)

- ✓ `OcrService::Anthropic` (default) and `OcrService::Google` (Document AI) for scanned PDF OCR — v1.2
- ✓ `--ocr-provider` / `LOCAL_INDEX_OCR_PROVIDER` to select provider; startup validation with clear errors — v1.2
- ✓ JWT service-account OAuth for Google Document AI; single retry on HTTP 429 — v1.2

**Phase 9: Preprocessor foundation** (validated 2026-04-15)

- ✓ `index` and `daemon` process configured asset extensions with optional `--skip-asset-processing` — v1.2
- ✓ Gitignore-aware asset discovery (PRE-03); PDF classification TextFirst/NeedsVision (PRE-05/PRE-06) — v1.2
- ✓ Anthropic vision client for standalone images; PDF rasterization via pdfium / pdftoppm fallback — v1.2
- ✓ Ephemeral cache path layout `asset-cache/{shard}/{sha256}.txt` under data dir — v1.2
- ✓ No companion `.processed.md` files written to vault; chunks stored with source asset file_path — v1.2
- ✓ `ChunkStore::prune_absent_markdown_files` skips non-.md paths so asset chunks survive prune — v1.2
- ✓ Credential resolution for Anthropic reuses `ANTHROPIC_API_KEY` pattern with clear errors (PRE-14) — v1.2

**Phase 6: Claude Code Integration** (validated 2026-04-13)

- [x] Claude Code skill files for search, reindex, and status (`.claude/skills/`)
- [x] Documented shell wrapper scripts for search, reindex, and status
- [x] README section documenting Claude Code integration

**Phase 7: Operational Logging** (validated 2026-04-14)

- [x] Every search query logged at INFO with `query`, `mode`, `results_returned`, `latency_ms` (CLI: `search completed`; web: `web search completed`)
- [x] Daemon file-watcher events logged at INFO with event type, path, `renamed_to` when applicable; indexing outcome with chunks added/removed/skipped
- [x] LanceDB/Lance internal tracing suppressed below WARN in default `EnvFilter` (`lancedb=warn,lance=warn`); full override when `RUST_LOG` is set

**Phase 8: Search UX Enhancements** (validated 2026-04-14)

- [x] Search UI exposes a "Rerank results" checkbox; checked submits `rerank=true`; unchecked submits `no_rerank=true` via disabled hidden field; `(reranked)` badge when reranking ran; disabled state + tooltip + Settings link when reranker unavailable
- [x] Snippet body highlights query terms (case-insensitive, word boundaries) with `<mark>`; HTML entity encoding on all user-controlled text segments

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

### Active

*(Awaiting next milestone definition — run `/gsd-new-milestone` to plan v1.3)*

### Out of Scope

- In-core PDF parsing inside `local-index` indexer — v1.2 routes PDFs through the asset pipeline; indexer processes `.md` chunks only
- DOCX ingestion — deferred (`FMT-02`)
- Remote/cloud LanceDB — embedded only; keeps deployment simple
- Authentication on the WebUI — local tool, no auth needed
- Settings UI in the WebUI — all config via CLI/.env; read-only view is sufficient
- Multi-vault support — single directory root per daemon instance
- Native PDF indexing (FMT-01) — companion `.processed.md` approach (SEED-001) is the supported path

## Context

- **Obsidian vault** is the primary data source: markdown files with YAML frontmatter, wiki-links (`[[note]]`), and nested heading structure
- **LanceDB** is embedded (no separate server), Rust-native via the `lancedb` crate
- **Voyage AI** (`voyage-3.5`, 1024 dims) is the embedding source; `VOYAGE_API_KEY` required
- **Anthropic** is used for vision/OCR (asset pipeline) and optional reranking; `ANTHROPIC_API_KEY` optional for markdown-only vaults
- **Primary consumer is Claude Code** via skill invocation — output must be machine-parseable JSON
- The web dashboard is a secondary consumer for human browsing and debugging
- **Current state:** ~8,500 LOC Rust (src/). 118 lib tests passing. Wiremock integration tests for Voyage + Anthropic. pdfium-render + lopdf for PDF processing; graceful degradation when libpdfium not installed.

## Constraints

- **Tech stack**: Rust only — no Node/Python helpers
- **Embeddings**: Voyage AI API only (voyage-3.5); no local models, no OpenAI
- **Database**: LanceDB embedded — no external database process
- **CLI framework**: `clap` with derive macros
- **Logging**: `tracing` crate (no `log` crate directly)
- **Metrics**: Prometheus-compatible `/metrics` endpoint; HDR histograms for latency
- **Deployment**: Single binary, runs on macOS (primary), Linux (secondary)

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Chunk by heading | More precise search results; Obsidian notes are heading-structured | Validated Phase 1 — smart size-based chunking with heading breadcrumbs |
| Voyage AI for embeddings | voyage-3.5 chosen over Anthropic; 1024 dims, better retrieval quality | Validated Phase 2 — VOYAGE_API_KEY env var, 50/batch, 5x retry |
| Env var preferred for credentials | Explicit over implicit; simpler than ~/.claude/ JSON parsing | Validated Phase 2 — VOYAGE_API_KEY only, clear error on missing |
| Single binary, embedded LanceDB | Zero deployment friction; no separate database process | Validated Phase 2 — 10-col Arrow schema, hash-based incremental indexing |
| Both daemon + one-shot modes | Daemon for real-time watching; one-shot for CI/manual re-index | Validated Phase 4 — notify + debouncer, tokio runtime, graceful shutdown |
| Prometheus + HDR histograms | Standard observability; histograms capture tail latency for API calls | Validated Phase 4 — /metrics endpoint, HDR histograms, 4 counter + 4 gauge metrics |
| Claude Code skills + shell wrappers | Skills for Claude integration; shell wrappers for humans and scripts | Validated Phase 6 — search/reindex/status skills + wrappers in repo |
| No companion .md files in vault | Keeping vault clean; derived text lives in LanceDB only, not visible files | Validated Phase 9 — asset chunks stored with source file_path; prune fix to protect them |
| SHA-256 cache-read gate before extension branch | Idempotent re-indexing; hash source bytes once, short-circuit all API on cache hit | Validated Phase 11 — `read_cache_if_present` in cache.rs; zero API calls on unchanged sources |
| Ephemeral cache in data dir, not vault | Cache is implementation detail, not user data; no Obsidian clutter | Validated Phase 9/11 — `asset-cache/{shard}/{sha256}.txt` under `LOCAL_INDEX_DATA_DIR` |
| Graceful degradation for pdfium | pdfium requires native library install; optional to avoid blocking macOS/Linux users | Validated Phase 11 — text-only fallback when `bind_to_system_library()` fails; WARN emitted |
| OcrService enum dispatch | Decouples PDF OCR from image vision; enables Google Document AI as drop-in | Validated Phase 10 — `OcrService::{Anthropic, Google}` with provider selection at startup |
| Pin `image` crate to 0.25.4 | 0.25.10 pulled `png` 0.18 pre-release and broke cargo build | Validated Phase 9 — pinned in Cargo.toml to avoid broken transitive dep |

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
*Last updated: 2026-04-20 after v1.2 milestone*
