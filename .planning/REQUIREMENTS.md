# Requirements: local-index

**Defined:** 2026-04-08
**Core Value:** Fast, accurate semantic search over a local markdown vault that Claude can query as a skill without any manual intervention.

## v1 Requirements

### CLI & Configuration

- [ ] **CLI-01**: Operator can run `local-index index <path>` to perform a one-shot full index of a directory tree and exit on completion
- [ ] **CLI-02**: Operator can run `local-index daemon <path>` to start a persistent background process that watches for file changes and indexes them in real time
- [ ] **CLI-03**: Operator can run `local-index search "<query>"` to perform a search and receive structured JSON results on stdout
- [ ] **CLI-04**: Operator can run `local-index status` to see total indexed chunks/files, last index time, pending queue depth, and stale file count
- [ ] **CLI-05**: Operator can run `local-index serve` to start the HTTP server (web dashboard + metrics) without the file watcher
- [x] **CLI-06**: All CLI commands and flags are implemented with `clap` derive macros and provide useful `--help` output with examples
- [x] **CLI-07**: All settings are configurable via CLI flags, `.env` file, and environment variables — no config file required for basic operation
- [x] **CLI-08**: CLI emits structured logging via the `tracing` crate; log level configurable via `RUST_LOG` or `--log-level` flag

### Credentials & Auth

- [ ] **CRED-01**: Credential resolution checks `ANTHROPIC_API_KEY` env var first, then falls back to parsing `~/.claude/` credential store
- [ ] **CRED-02**: Embedding provider is configurable via an `Embedder` trait; v1 ships Voyage AI (via Anthropic key) and Google Gemini implementations
- [ ] **CRED-03**: Startup fails with a clear error message if no valid credentials are found for the configured embedding provider

### Indexing Pipeline

- [x] **INDX-01**: Indexer walks a directory tree recursively and processes all `.md` files; non-markdown files are skipped with a trace log
- [x] **INDX-02**: Markdown files are chunked by heading: each heading section (heading + its body text) becomes one embedding unit; heading hierarchy is preserved as a breadcrumb (e.g., `## Goals > ### Q1`)
- [x] **INDX-03**: YAML frontmatter is stripped from chunk text before embedding but stored as structured metadata (tags, aliases, dates) on each chunk record
- [ ] **INDX-04**: Each chunk's content is SHA-256 hashed; on re-index, only chunks whose hash has changed are re-embedded (unchanged chunks are skipped)
- [ ] **INDX-05**: Embeddings are stored in embedded LanceDB alongside chunk text, heading breadcrumb, file path (vault-relative), line range (start/end line numbers), frontmatter metadata, content hash, and embedding model ID
- [ ] **INDX-06**: When the configured embedding model ID differs from the model ID stored in the database, the indexer warns the operator and requires `--force-reindex` to proceed
- [ ] **INDX-07**: Embedding API calls use exponential backoff with jitter on rate-limit or transient errors; failed chunks are queued for retry without losing already-indexed data
- [ ] **INDX-08**: Indexer reports progress during one-shot mode (files processed, chunks embedded, errors)

### File Watching (Daemon Mode)

- [ ] **WTCH-01**: Daemon mode uses the `notify` crate (with debounce via `notify-debouncer-full`) to watch the target directory recursively for create, modify, rename, and delete events
- [ ] **WTCH-02**: File rename events are handled as delete-old-path + index-new-path (path change invalidates all chunks for the old path)
- [ ] **WTCH-03**: File delete events remove all chunks for that file from the index
- [ ] **WTCH-04**: The file watcher, embedding pipeline, and HTTP server run concurrently in a single tokio runtime; graceful shutdown is coordinated via a broadcast channel on SIGINT/SIGTERM

### Search

- [ ] **SRCH-01**: Search supports semantic (vector ANN) queries via LanceDB's native vector search
- [ ] **SRCH-02**: Search supports full-text queries over chunk text
- [ ] **SRCH-03**: Search supports hybrid mode that fuses semantic and full-text scores via Reciprocal Rank Fusion (RRF); hybrid is the default search mode
- [ ] **SRCH-04**: Search results are returned as structured JSON with fields: `chunk_text`, `file_path` (vault-relative), `heading_breadcrumb`, `similarity_score`, `line_range` (start/end), `frontmatter` (tags/aliases/date)
- [ ] **SRCH-05**: Search supports `--limit N` (default: 10) and `--min-score F` (default: none) flags
- [ ] **SRCH-06**: Search supports `--mode [semantic|fts|hybrid]` flag to select search mode explicitly
- [ ] **SRCH-07**: Search supports `--path-filter <prefix>` to restrict results to files under a given path prefix
- [ ] **SRCH-08**: Search supports `--tag-filter <tag>` to restrict results to chunks from files with a given frontmatter tag
- [ ] **SRCH-09**: Search supports `--format [json|pretty]` flag; `json` is the default (machine-readable), `pretty` renders a human-readable table
- [ ] **SRCH-10**: Search supports `--context N` flag (default: 0) to include N chunks before and after each matching chunk in results

### Observability

- [ ] **OBS-01**: The HTTP server exposes a Prometheus-compatible `/metrics` endpoint
- [ ] **OBS-02**: HDR histogram (or equivalent high-resolution histogram) metrics are tracked for: embedding API call latency, indexing throughput (chunks/sec), search query latency, HTTP request latency
- [ ] **OBS-03**: Counter metrics are tracked for: total chunks indexed, embedding API errors, file events processed, search queries served
- [ ] **OBS-04**: Gauge metrics are tracked for: current queue depth (pending embeds), total chunks in index, total files in index, stale file count

### Web Dashboard

- [ ] **WEB-01**: The HTTP server serves a web dashboard on a configurable port (default: 3000); binds to `127.0.0.1` by default; `--bind` flag allows overriding
- [ ] **WEB-02**: Dashboard includes a search UI: text input, search mode selector, results list with chunk text, file path, heading breadcrumb, and similarity score
- [ ] **WEB-03**: Dashboard includes an index browser: list of all indexed files with per-file chunk count and last-indexed timestamp
- [ ] **WEB-04**: Dashboard includes an index status view: total chunks, total files, last full-index time, pending queue depth, stale file count
- [ ] **WEB-05**: Dashboard includes an embedding stats view: total embeddings, embedding model ID, estimated token usage (if available from API response)
- [ ] **WEB-06**: Dashboard includes a read-only settings view: current config values, credential source (env var vs `~/.claude/`), active CLI flags

### Claude Code Integration

- [ ] **INTG-01**: A Claude Code skill file (`.claude/skills/search.md`) is shipped that enables Claude Code to invoke `local-index search` and parse the JSON results
- [ ] **INTG-02**: A Claude Code skill file (`.claude/skills/reindex.md`) is shipped that enables Claude Code to trigger a one-shot re-index
- [ ] **INTG-03**: A Claude Code skill file (`.claude/skills/status.md`) is shipped that enables Claude Code to check index status
- [ ] **INTG-04**: Documented shell wrapper scripts for `search`, `reindex`, and `status` are included in the repository

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

| Requirement | Phase | Status |
|-------------|-------|--------|
| CLI-01 | Phase 2 | Pending |
| CLI-02 | Phase 4 | Pending |
| CLI-03 | Phase 3 | Pending |
| CLI-04 | Phase 4 | Pending |
| CLI-05 | Phase 5 | Pending |
| CLI-06 | Phase 1 | Complete |
| CLI-07 | Phase 1 | Complete |
| CLI-08 | Phase 1 | Complete |
| CRED-01 | Phase 2 | Pending |
| CRED-02 | Phase 2 | Pending |
| CRED-03 | Phase 2 | Pending |
| INDX-01 | Phase 1 | Complete |
| INDX-02 | Phase 1 | Complete |
| INDX-03 | Phase 1 | Complete |
| INDX-04 | Phase 2 | Pending |
| INDX-05 | Phase 2 | Pending |
| INDX-06 | Phase 2 | Pending |
| INDX-07 | Phase 2 | Pending |
| INDX-08 | Phase 2 | Pending |
| WTCH-01 | Phase 4 | Pending |
| WTCH-02 | Phase 4 | Pending |
| WTCH-03 | Phase 4 | Pending |
| WTCH-04 | Phase 4 | Pending |
| SRCH-01 | Phase 3 | Pending |
| SRCH-02 | Phase 3 | Pending |
| SRCH-03 | Phase 3 | Pending |
| SRCH-04 | Phase 3 | Pending |
| SRCH-05 | Phase 3 | Pending |
| SRCH-06 | Phase 3 | Pending |
| SRCH-07 | Phase 3 | Pending |
| SRCH-08 | Phase 3 | Pending |
| SRCH-09 | Phase 3 | Pending |
| SRCH-10 | Phase 3 | Pending |
| OBS-01 | Phase 4 | Pending |
| OBS-02 | Phase 4 | Pending |
| OBS-03 | Phase 4 | Pending |
| OBS-04 | Phase 4 | Pending |
| WEB-01 | Phase 5 | Pending |
| WEB-02 | Phase 5 | Pending |
| WEB-03 | Phase 5 | Pending |
| WEB-04 | Phase 5 | Pending |
| WEB-05 | Phase 5 | Pending |
| WEB-06 | Phase 5 | Pending |
| INTG-01 | Phase 6 | Pending |
| INTG-02 | Phase 6 | Pending |
| INTG-03 | Phase 6 | Pending |
| INTG-04 | Phase 6 | Pending |

**Coverage:**
- v1 requirements: 45 total
- Mapped to phases: 45
- Unmapped: 0

---
*Requirements defined: 2026-04-08*
*Last updated: 2026-04-08 after roadmap creation*
