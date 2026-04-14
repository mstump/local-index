# Requirements: local-index

**Defined:** 2026-04-08
**Core Value:** Fast, accurate semantic search over a local markdown vault that Claude can query as a skill without any manual intervention.

## v1 Requirements

### CLI & Configuration

- [x] **CLI-01**: Operator can run `local-index index <path>` to perform a one-shot full index of a directory tree and exit on completion
- [x] **CLI-02**: Operator can run `local-index daemon <path>` to start a persistent background process that watches for file changes and indexes them in real time
- [x] **CLI-03**: Operator can run `local-index search "<query>"` to perform a search and receive structured JSON results on stdout
- [x] **CLI-04**: Operator can run `local-index status` to see total indexed chunks/files, last index time, pending queue depth, and stale file count
- [x] **CLI-05**: Operator can run `local-index serve` to start the HTTP server (web dashboard + metrics) without the file watcher
- [x] **CLI-06**: All CLI commands and flags are implemented with `clap` derive macros and provide useful `--help` output with examples
- [x] **CLI-07**: All settings are configurable via CLI flags, `.env` file, and environment variables — no config file required for basic operation
- [x] **CLI-08**: CLI emits structured logging via the `tracing` crate; log level configurable via `RUST_LOG` or `--log-level` flag

### Credentials & Auth

- [x] **CRED-01**: Credential resolution checks `VOYAGE_API_KEY` env var for the configured embedding provider (Voyage AI in v1). `~/.claude/` fallback is deferred — not applicable to Voyage AI credentials.
- [x] **CRED-02**: Embedding provider is configurable via an `Embedder` trait; v1 ships Voyage AI implementation. Google Gemini implementation is deferred to a future phase.
- [x] **CRED-03**: Startup fails with a clear error message if no valid credentials are found for the configured embedding provider

### Indexing Pipeline

- [x] **INDX-01**: Indexer walks a directory tree recursively and processes all `.md` files; non-markdown files are skipped with a trace log
- [x] **INDX-02**: Markdown files are chunked by heading: each heading section (heading + its body text) becomes one embedding unit; heading hierarchy is preserved as a breadcrumb (e.g., `## Goals > ### Q1`)
- [x] **INDX-03**: YAML frontmatter is stripped from chunk text before embedding but stored as structured metadata (tags, aliases, dates) on each chunk record
- [x] **INDX-04**: Each chunk's content is SHA-256 hashed; on re-index, only chunks whose hash has changed are re-embedded (unchanged chunks are skipped)
- [x] **INDX-05**: Embeddings are stored in embedded LanceDB alongside chunk text, heading breadcrumb, file path (vault-relative), line range (start/end line numbers), frontmatter metadata, content hash, and embedding model ID
- [x] **INDX-06**: When the configured embedding model ID differs from the model ID stored in the database, the indexer warns the operator and requires `--force-reindex` to proceed
- [x] **INDX-07**: Embedding API calls use exponential backoff with jitter on rate-limit or transient errors; failed chunks are queued for retry without losing already-indexed data
- [x] **INDX-08**: Indexer reports progress during one-shot mode (files processed, chunks embedded, errors)

### File Watching (Daemon Mode)

- [x] **WTCH-01**: Daemon mode uses the `notify` crate (with debounce via `notify-debouncer-full`) to watch the target directory recursively for create, modify, rename, and delete events
- [x] **WTCH-02**: File rename events are handled as delete-old-path + index-new-path (path change invalidates all chunks for the old path)
- [x] **WTCH-03**: File delete events remove all chunks for that file from the index
- [x] **WTCH-04**: The file watcher, embedding pipeline, and HTTP server run concurrently in a single tokio runtime; graceful shutdown is coordinated via a broadcast channel on SIGINT/SIGTERM

### Search

- [x] **SRCH-01**: Search supports semantic (vector ANN) queries via LanceDB's native vector search
- [x] **SRCH-02**: Search supports full-text queries over chunk text
- [x] **SRCH-03**: Search supports hybrid mode that fuses semantic and full-text scores via Reciprocal Rank Fusion (RRF); hybrid is the default search mode
- [x] **SRCH-04**: Search results are returned as structured JSON with fields: `chunk_text`, `file_path` (vault-relative), `heading_breadcrumb`, `similarity_score`, `line_range` (start/end), `frontmatter` (tags/aliases/date)
- [x] **SRCH-05**: Search supports `--limit N` (default: 10) and `--min-score F` (default: none) flags
- [x] **SRCH-06**: Search supports `--mode [semantic|fts|hybrid]` flag to select search mode explicitly
- [x] **SRCH-07**: Search supports `--path-filter <prefix>` to restrict results to files under a given path prefix
- [x] **SRCH-08**: Search supports `--tag-filter <tag>` to restrict results to chunks from files with a given frontmatter tag
- [x] **SRCH-09**: Search supports `--format [json|pretty]` flag; `json` is the default (machine-readable), `pretty` renders a human-readable table
- [x] **SRCH-10**: Search supports `--context N` flag (default: 0) to include N chunks before and after each matching chunk in results

### Observability

- [x] **OBS-01**: The HTTP server exposes a Prometheus-compatible `/metrics` endpoint
- [x] **OBS-02**: HDR histogram (or equivalent high-resolution histogram) metrics are tracked for: embedding API call latency, indexing throughput (chunks/sec), search query latency, HTTP request latency
- [x] **OBS-03**: Counter metrics are tracked for: total chunks indexed, embedding API errors, file events processed, search queries served
- [x] **OBS-04**: Gauge metrics are tracked for: current queue depth (pending embeds), total chunks in index, total files in index, stale file count

### Web Dashboard

- [x] **WEB-01**: The HTTP server serves a web dashboard on a configurable port (default: 3000); binds to `127.0.0.1` by default; `--bind` flag allows overriding
- [x] **WEB-02**: Dashboard includes a search UI: text input, search mode selector, results list with chunk text, file path, heading breadcrumb, and similarity score
- [x] **WEB-03**: Dashboard includes an index browser: list of all indexed files with per-file chunk count and last-indexed timestamp
- [x] **WEB-04**: Dashboard includes an index status view: total chunks, total files, last full-index time, pending queue depth, stale file count
- [x] **WEB-05**: Dashboard includes an embedding stats view: total embeddings, embedding model ID, estimated token usage (if available from API response)
- [x] **WEB-06**: Dashboard includes a read-only settings view: current config values, credential source (env var), active CLI flags

### Claude Code Integration

- [x] **INTG-01**: A Claude Code skill file (`.claude/skills/search.md`) is shipped that enables Claude Code to invoke `local-index search` and parse the JSON results
- [x] **INTG-02**: A Claude Code skill file (`.claude/skills/reindex.md`) is shipped that enables Claude Code to trigger a one-shot re-index
- [x] **INTG-03**: A Claude Code skill file (`.claude/skills/status.md`) is shipped that enables Claude Code to check index status
- [x] **INTG-04**: Documented shell wrapper scripts for `search`, `reindex`, and `status` are included in the repository

---

## v1.1 Requirements

### Web UI

- [x] **WEB-07**: Search UI includes a "Rerank results" checkbox; when checked the search request includes `rerank=true`; results display a "(reranked)" indicator; checkbox is disabled when `ANTHROPIC_API_KEY` is not set
- [x] **WEB-08**: Search result snippets highlight all query terms (case-insensitive, word boundary match) using `<mark>` elements in the displayed chunk_text

### Logging

- [x] **LOG-01**: Every search query is logged at INFO level with structured fields: `query`, `mode`, `results_returned`, `latency_ms`
- [x] **LOG-02**: Daemon file-watcher events are logged at INFO level with fields: `event` (Created/Modified/Renamed/Deleted), `path`, and `renamed_to` (rename events only); indexing outcome (chunks added/removed/skipped) logged after each event is processed
- [x] **LOG-03**: LanceDB internal tracing output is suppressed below WARN via `EnvFilter` directive (`lancedb=warn,lance=warn`), removing verbose source-path messages without losing actionable warnings

---

## v2 Requirements

### Document Formats

- **FMT-01**: Indexer processes PDF files (requires poppler or similar extraction)
- **FMT-02**: Indexer processes DOCX files
- **FMT-03**: Non-text files (images, audio) are skipped with a warning but do not crash the indexer

### Wiki-Link Resolution

- **LINK-01**: Obsidian `[[note]]` and `[[note|alias]]` links are parsed and stored as outbound link metadata on each chunk
- **LINK-02**: Search supports `--related <file>` mode that finds chunks linking to a given note

### Chunk Overlap

- **CHNK-01**: Indexer supports `--chunk-overlap N` flag (default: 0) to repeat the last N sentences of the previous chunk at the start of the next chunk within the same heading section

### Additional Embedding Providers

- **EMBD-01**: OpenAI `text-embedding-3-*` models supported as a configurable `Embedder` implementation
- **EMBD-02**: Local embedding model support via ONNX runtime (no external API dependency)

---

## Out of Scope

| Feature | Reason |
|---------|--------|
| Multi-vault / multi-tenant | One daemon per vault is simple and correct; users run multiple instances for multiple vaults |
| LLM-powered query rewriting | Claude Code is already an LLM; raw results are better — let the caller decide how to requery |
| Semantic caching / query result cache | LanceDB ANN search on a personal vault (<100K chunks) is fast enough; caching adds staleness bugs |
| Real-time collaborative editing awareness | Obsidian is single-user; sync conflicts are handled as normal file change events |
| GUI configuration editor | Config via CLI/.env is the right UX; settings UI is maintenance burden with near-zero usage |
| Plugin / extension system | Premature abstraction; internal traits provide extensibility for contributors |
| WebUI authentication | Local daemon on 127.0.0.1; auth adds complexity with no security benefit |
| Automatic chunk summarization | Multiplies API cost 10x; the chunk text is the summary; Claude Code can summarize results |

---

## Traceability

### v1 (all complete -- 2026-04-13)

| Requirement | Phase | Status |
|-------------|-------|--------|
| CLI-01 | Phase 2 | Complete |
| CLI-02 | Phase 4 | Complete |
| CLI-03 | Phase 3 | Complete |
| CLI-04 | Phase 4 | Complete |
| CLI-05 | Phase 5 | Complete |
| CLI-06 | Phase 1 | Complete |
| CLI-07 | Phase 1 | Complete |
| CLI-08 | Phase 1 | Complete |
| CRED-01 | Phase 2 | Complete |
| CRED-02 | Phase 2 | Complete |
| CRED-03 | Phase 2 | Complete |
| INDX-01 | Phase 1 | Complete |
| INDX-02 | Phase 1 | Complete |
| INDX-03 | Phase 1 | Complete |
| INDX-04 | Phase 2 | Complete |
| INDX-05 | Phase 2 | Complete |
| INDX-06 | Phase 2 | Complete |
| INDX-07 | Phase 2 | Complete |
| INDX-08 | Phase 2 | Complete |
| WTCH-01 | Phase 4 | Complete |
| WTCH-02 | Phase 4 | Complete |
| WTCH-03 | Phase 4 | Complete |
| WTCH-04 | Phase 4 | Complete |
| SRCH-01 | Phase 3 | Complete |
| SRCH-02 | Phase 3 | Complete |
| SRCH-03 | Phase 3 | Complete |
| SRCH-04 | Phase 3 | Complete |
| SRCH-05 | Phase 3 | Complete |
| SRCH-06 | Phase 3 | Complete |
| SRCH-07 | Phase 3 | Complete |
| SRCH-08 | Phase 3 | Complete |
| SRCH-09 | Phase 3 | Complete |
| SRCH-10 | Phase 3 | Complete |
| OBS-01 | Phase 4 | Complete |
| OBS-02 | Phase 4 | Complete |
| OBS-03 | Phase 4 | Complete |
| OBS-04 | Phase 4 | Complete |
| WEB-01 | Phase 5 | Complete |
| WEB-02 | Phase 5 | Complete |
| WEB-03 | Phase 5 | Complete |
| WEB-04 | Phase 5 | Complete |
| WEB-05 | Phase 5 | Complete |
| WEB-06 | Phase 5 | Complete |
| INTG-01 | Phase 6 | Complete |
| INTG-02 | Phase 6 | Complete |
| INTG-03 | Phase 6 | Complete |
| INTG-04 | Phase 6 | Complete |

### v1.1 (in progress)

| Requirement | Phase | Status |
|-------------|-------|--------|
| LOG-01 | Phase 7 | Complete |
| LOG-02 | Phase 7 | Complete |
| LOG-03 | Phase 7 | Complete |
| WEB-07 | Phase 8 | Satisfied |
| WEB-08 | Phase 8 | Satisfied |

**Coverage:**
- v1 requirements: 45/45 complete
- v1.1 requirements: 5/5 mapped (Phase 7: 3, Phase 8: 2)

---
*Requirements defined: 2026-04-08*
*Last updated: 2026-04-14 -- v1.1 requirements mapped to phases (LOG-01-03 -> Phase 7, WEB-07-08 -> Phase 8)*
