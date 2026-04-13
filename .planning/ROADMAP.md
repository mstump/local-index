# Roadmap: local-index

## Overview

local-index delivers a single Rust binary that watches a markdown vault, chunks using smart size-based splitting with semantic break-point detection (inspired by qmd), embeds via configurable providers, stores in embedded LanceDB, and exposes hybrid search through CLI, web dashboard, and Claude Code skills. The build progresses from zero-dependency foundation (CLI + chunker) through storage and embedding integration, search capabilities, daemon mode with observability, web dashboard, and finally Claude Code integration -- each phase independently verifiable.

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

- [ ] **Phase 1: Foundation & File Processing** - CLI skeleton, markdown chunking, config resolution, structured logging
- [ ] **Phase 2: Storage & Embedding Pipeline** - LanceDB integration, Embedder trait, credential resolution, one-shot indexing
- [x] **Phase 3: Search** - Semantic, full-text, and hybrid search with all query flags and output formats (completed 2026-04-10)
- [ ] **Phase 4: Daemon Mode & Observability** - File watcher, concurrent runtime, Prometheus metrics, graceful shutdown
- [ ] **Phase 5: Web Dashboard** - axum-served dashboard with search UI, index browser, status views
- [ ] **Phase 6: Claude Code Integration** - Skill files, shell wrappers, serve command

## Phase Details

### Phase 1: Foundation & File Processing
**Goal**: Operator can parse and chunk a markdown vault from the command line with full structured logging
**Depends on**: Nothing (first phase)
**Requirements**: CLI-06, CLI-07, CLI-08, INDX-01, INDX-02, INDX-03
**Success Criteria** (what must be TRUE):
  1. Operator can run the binary and see valid `--help` output with all subcommands listed
  2. Operator can point the tool at a directory and it recursively discovers all `.md` files, skipping non-markdown with a trace log
  3. Each markdown file is chunked using a smart size-based algorithm (≤3600 chars/chunk, 15% overlap, break points scored by semantic quality: h1=100, h2=90, h3/codeblock=80…blank=20, no splits inside code fences); heading hierarchy is preserved as a breadcrumb metadata field on each chunk (e.g., `## Goals > ### Q1`) — **Note: chunker rewritten in Phase 2 Plan 01 to use this algorithm**
  4. YAML frontmatter is stripped from chunk content but available as structured metadata (tags, aliases, dates)
  5. Log output is structured via `tracing` and controllable via `RUST_LOG` or `--log-level`
**Plans**: 3 plans

Plans:
- [x] 01-01: TBD
- [x] 01-02: TBD
- [x] 01-03: TBD

### Phase 2: Storage & Embedding Pipeline
**Goal**: Operator can index a vault end-to-end with embeddings stored in LanceDB, with incremental re-indexing on unchanged content
**Depends on**: Phase 1
**Requirements**: CLI-01, CRED-01, CRED-02, CRED-03, INDX-04, INDX-05, INDX-06, INDX-07, INDX-08
**Success Criteria** (what must be TRUE):
  1. Operator can run `local-index index <path>` and the tool embeds all chunks and stores them in LanceDB, then exits
  2. Credential resolution finds API key from env var first, then `~/.claude/` fallback; startup fails with clear error if no credentials found
  3. Re-running index on an unchanged vault skips all chunks (SHA-256 content hash match); only changed chunks are re-embedded
  4. When the configured embedding model differs from what is stored in the database, the tool warns and requires `--force-reindex`
  5. Transient API errors trigger exponential backoff with jitter; partial failures do not lose already-indexed data
**Plans**: 3 plans

Plans:
- [x] 02-01-PLAN.md — Dependencies, credentials, Embedder trait, VoyageEmbedder with retry
- [x] 02-02-PLAN.md — LanceDB ChunkStore with schema, upsert, hash query, model guard
- [x] 02-03-PLAN.md — Wire index command to embed+store pipeline with progress reporting

### Phase 3: Search
**Goal**: Operator can search their indexed vault with semantic, full-text, or hybrid queries and receive structured results
**Depends on**: Phase 2
**Requirements**: CLI-03, SRCH-01, SRCH-02, SRCH-03, SRCH-04, SRCH-05, SRCH-06, SRCH-07, SRCH-08, SRCH-09, SRCH-10
**Reference**: qmd (tobi/qmd) — strong prior art for the hybrid search approach
**Success Criteria** (what must be TRUE):
  1. Operator can run `local-index search "<query>"` and receive JSON results with chunk_text, file_path, heading_breadcrumb, similarity_score, line_range, and frontmatter
  2. Hybrid mode (default) fuses BM25 full-text and vector semantic results via Reciprocal Rank Fusion (RRF); operator can switch to pure semantic or pure FTS via `--mode`
  3. Operator can filter results by path prefix (`--path-filter`) and frontmatter tag (`--tag-filter`)
  4. Operator can control result count (`--limit`), minimum score (`--min-score`), context window (`--context`), and output format (`--format json|pretty`)
**Search design notes** (from qmd analysis):
  - Three query types: `lex` (keyword → BM25/FTS only), `vec` (semantic → vector only), `hyde` (hypothetical document expansion → vector only)
  - `query` command (default) uses typed expansion: generate lex+vec+hyde variants, run all, fuse with RRF, return ranked results
  - `search` command: BM25-only (fast, keyword)
  - `vsearch` command: vector-only (semantic)
  - LanceDB FTS (confirmed available in Rust crate) handles BM25; LanceDB vector search handles semantic
  - Skip LLM reranking in v1 (qmd uses local GGUF models — not our deployment model)
**Plans**: 2 plans

Plans:
- [x] 03-01-PLAN.md — Search module: types, SearchEngine with semantic/FTS/hybrid modes, score normalization
- [x] 03-02-PLAN.md — Output formatters, CLI wiring, FTS index in index command, integration tests

### Phase 4: Daemon Mode & Observability
**Goal**: Operator can run a persistent daemon that watches for file changes and re-indexes in real time, with full Prometheus metrics
**Depends on**: Phase 2
**Requirements**: CLI-02, CLI-04, WTCH-01, WTCH-02, WTCH-03, WTCH-04, OBS-01, OBS-02, OBS-03, OBS-04
**Success Criteria** (what must be TRUE):
  1. Operator can run `local-index daemon <path>` and the process watches for file create/modify/rename/delete events, re-indexing affected chunks automatically
  2. File renames are handled as delete-old + index-new; file deletes remove all chunks for that file
  3. Operator can run `local-index status` and see total chunks, files, last index time, pending queue depth, and stale file count
  4. A `/metrics` endpoint serves Prometheus-compatible metrics including HDR histograms for embedding latency, indexing throughput, search latency, and HTTP latency
  5. Graceful shutdown on SIGINT/SIGTERM completes in-flight work without data loss
**Plans**: 3 plans

Plans:
- [x] 04-01-PLAN.md — Dependencies, metrics foundation, Prometheus setup, HTTP router
- [x] 04-02-PLAN.md — Status command with ChunkStore aggregate queries
- [x] 04-03-PLAN.md — File watcher, event processor, graceful shutdown, daemon CLI wiring

### Phase 5: Web Dashboard
**Goal**: Operator can browse and search their index through a web interface served by the same process
**Depends on**: Phase 3, Phase 4
**Requirements**: CLI-05, WEB-01, WEB-02, WEB-03, WEB-04, WEB-05, WEB-06
**Success Criteria** (what must be TRUE):
  1. Operator can run `local-index serve` or `local-index daemon` and open a web dashboard at `http://127.0.0.1:3000` (port configurable via `--bind`)
  2. Dashboard search UI accepts a query, lets the operator select search mode, and displays ranked results with chunk text, file path, breadcrumb, and score
  3. Dashboard index browser lists all indexed files with per-file chunk count and last-indexed timestamp
  4. Dashboard shows index status (total chunks/files, last index time, queue depth) and embedding stats (count, model, token usage)
  5. Dashboard shows a read-only settings view with current config values and credential source
**Plans**: 3 plans
**UI hint**: yes

Plans:
- [x] 05-01-PLAN.md — Foundation: askama deps, web module, AppState, dashboard router, serve command, base template
- [x] 05-02-PLAN.md — Search page: search handler with SearchEngine, search template with form/results/empty states
- [x] 05-03-PLAN.md — Index browser, status page with embedding stats, settings page

### Phase 6: Claude Code Integration
**Goal**: Claude Code can invoke search, re-index, and status checks via skill files without human intervention
**Depends on**: Phase 3
**Requirements**: INTG-01, INTG-02, INTG-03, INTG-04
**Success Criteria** (what must be TRUE):
  1. A `.claude/skills/search.md` skill file exists that enables Claude Code to invoke `local-index search` and parse JSON results
  2. Skill files for `reindex` and `status` exist and work correctly when invoked by Claude Code
  3. Shell wrapper scripts for search, reindex, and status are included in the repository and documented
**Plans**: 1 plan

Plans:
- [ ] 06-01-PLAN.md — Skill files (search, reindex, status), shell wrappers, README Claude Code Integration section

## Progress

**Execution Order:**
Phases execute in numeric order: 1 -> 2 -> 3 -> 4 -> 5 -> 6

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Foundation & File Processing | 3/3 | Complete | 2026-04-09 |
| 2. Storage & Embedding Pipeline | 3/3 | Complete | 2026-04-10 |
| 3. Search | 2/2 | Complete   | 2026-04-10 |
| 4. Daemon Mode & Observability | 0/3 | Not started | - |
| 5. Web Dashboard | 0/3 | Not started | - |
| 6. Claude Code Integration | 0/1 | Not started | - |
