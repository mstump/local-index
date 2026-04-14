# Roadmap: local-index

## Milestones

- ✅ **v1.0 Core Indexer** - Phases 1-6 (shipped 2026-04-13)
- 🚧 **v1.1 Search UX & Observability** - Phases 7-8 (in progress)

## Phases

<details>
<summary>v1.0 Core Indexer (Phases 1-6) - SHIPPED 2026-04-13</summary>

- [x] **Phase 1: Foundation & File Processing** - CLI skeleton, markdown chunking, config resolution, structured logging
- [x] **Phase 2: Storage & Embedding Pipeline** - LanceDB integration, Embedder trait, credential resolution, one-shot indexing
- [x] **Phase 3: Search** - Semantic, full-text, and hybrid search with all query flags and output formats
- [x] **Phase 4: Daemon Mode & Observability** - File watcher, concurrent runtime, Prometheus metrics, graceful shutdown
- [x] **Phase 5: Web Dashboard** - axum-served dashboard with search UI, index browser, status views
- [x] **Phase 6: Claude Code Integration** - Skill files, shell wrappers, serve command

</details>

### v1.1 Search UX & Observability

- [x] **Phase 7: Operational Logging** - Structured search/daemon logging, LanceDB noise suppression (completed 2026-04-14)
- [ ] **Phase 8: Search UX Enhancements** - Reranking toggle and query term highlighting in web UI

## Phase Details

<details>
<summary>v1.0 Phase Details (Phases 1-6)</summary>

### Phase 1: Foundation & File Processing
**Goal**: Operator can parse and chunk a markdown vault from the command line with full structured logging
**Depends on**: Nothing (first phase)
**Requirements**: CLI-06, CLI-07, CLI-08, INDX-01, INDX-02, INDX-03
**Status**: Complete (2026-04-10)
**Plans**: 3 plans

Plans:
- [x] 01-01: CLI skeleton with clap derive, subcommands, global flags
- [x] 01-02: Markdown walker, YAML frontmatter parser, heading breadcrumbs
- [x] 01-03: Smart size-based chunker with semantic break-point scoring

### Phase 2: Storage & Embedding Pipeline
**Goal**: Operator can index a vault end-to-end with embeddings stored in LanceDB, with incremental re-indexing on unchanged content
**Depends on**: Phase 1
**Requirements**: CLI-01, CRED-01, CRED-02, CRED-03, INDX-04, INDX-05, INDX-06, INDX-07, INDX-08
**Status**: Complete (2026-04-10)
**Plans**: 3 plans

Plans:
- [x] 02-01: Dependencies, credentials, Embedder trait, VoyageEmbedder with retry
- [x] 02-02: LanceDB ChunkStore with schema, upsert, hash query, model guard
- [x] 02-03: Wire index command to embed+store pipeline with progress reporting

### Phase 3: Search
**Goal**: Operator can search their indexed vault with semantic, full-text, or hybrid queries and receive structured results
**Depends on**: Phase 2
**Requirements**: CLI-03, SRCH-01, SRCH-02, SRCH-03, SRCH-04, SRCH-05, SRCH-06, SRCH-07, SRCH-08, SRCH-09, SRCH-10
**Status**: Complete (2026-04-10)
**Plans**: 2 plans

Plans:
- [x] 03-01: Search module: types, SearchEngine with semantic/FTS/hybrid modes, score normalization
- [x] 03-02: Output formatters, CLI wiring, FTS index in index command, integration tests

### Phase 4: Daemon Mode & Observability
**Goal**: Operator can run a persistent daemon that watches for file changes and re-indexes in real time, with full Prometheus metrics
**Depends on**: Phase 2
**Requirements**: CLI-02, CLI-04, WTCH-01, WTCH-02, WTCH-03, WTCH-04, OBS-01, OBS-02, OBS-03, OBS-04
**Status**: Complete (2026-04-10)
**Plans**: 3 plans

Plans:
- [x] 04-01: Dependencies, metrics foundation, Prometheus setup, HTTP router
- [x] 04-02: Status command with ChunkStore aggregate queries
- [x] 04-03: File watcher, event processor, graceful shutdown, daemon CLI wiring

### Phase 5: Web Dashboard
**Goal**: Operator can browse and search their index through a web interface served by the same process
**Depends on**: Phase 3, Phase 4
**Requirements**: CLI-05, WEB-01, WEB-02, WEB-03, WEB-04, WEB-05, WEB-06
**Status**: Complete (2026-04-12)
**Plans**: 3 plans
**UI hint**: yes

Plans:
- [x] 05-01: Foundation: askama deps, web module, AppState, dashboard router, serve command, base template
- [x] 05-02: Search page: search handler with SearchEngine, search template with form/results/empty states
- [x] 05-03: Index browser, status page with embedding stats, settings page

### Phase 6: Claude Code Integration
**Goal**: Claude Code can invoke search, re-index, and status checks via skill files without human intervention
**Depends on**: Phase 3
**Requirements**: INTG-01, INTG-02, INTG-03, INTG-04
**Status**: Complete (2026-04-13)
**Plans**: 1 plan

Plans:
- [x] 06-01: Skill files (search, reindex, status), shell wrappers, README Claude Code Integration section

</details>

### Phase 7: Operational Logging
**Goal**: Operators can see what the daemon is doing from logs alone -- every search query and file event is visible at INFO level, and LanceDB noise is gone
**Depends on**: Phase 6 (v1.0 complete)
**Requirements**: LOG-01, LOG-02, LOG-03
**Success Criteria** (what must be TRUE):
  1. Running a search query (CLI or web) produces an INFO log line containing the query text, search mode, number of results returned, and latency in milliseconds
  2. When the daemon processes a file event (create, modify, rename, delete), an INFO log line appears with the event type, file path, and (for renames) the destination path; a follow-up log line shows the indexing outcome (chunks added, removed, or skipped)
  3. Running the daemon with default RUST_LOG settings produces no LanceDB/Lance internal trace messages (verbose source file paths, internal spans); setting RUST_LOG=lancedb=debug restores them on demand
**Plans**: 1 plan

Plans:
- [x] 07-01-PLAN.md — Search query logging, daemon event logging, LanceDB noise suppression

### Phase 8: Search UX Enhancements
**Goal**: The web search UI surfaces reranking controls and highlights matching terms so operators find relevant results faster
**Depends on**: Phase 7
**Requirements**: WEB-07, WEB-08
**Success Criteria** (what must be TRUE):
  1. The search page shows a "Rerank results" checkbox; checking it and searching sends rerank=true to the backend; results display a "(reranked)" badge; when ANTHROPIC_API_KEY is not set, the checkbox is visually disabled with a tooltip explaining why
  2. After searching, every occurrence of each query term in result snippets is wrapped in a visible highlight (case-insensitive, word-boundary aware); multi-word queries highlight each term independently
  3. Highlighting does not break HTML entities or inject raw HTML from user input (query terms are escaped before insertion into markup)
**Plans**: 1 plan
**UI hint**: yes

Plans:
- [ ] 08-01: TBD

## Progress

**Execution Order:** Phases 7 then 8.

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1. Foundation & File Processing | v1.0 | 3/3 | Complete | 2026-04-09 |
| 2. Storage & Embedding Pipeline | v1.0 | 3/3 | Complete | 2026-04-10 |
| 3. Search | v1.0 | 2/2 | Complete | 2026-04-10 |
| 4. Daemon Mode & Observability | v1.0 | 3/3 | Complete | 2026-04-10 |
| 5. Web Dashboard | v1.0 | 3/3 | Complete | 2026-04-12 |
| 6. Claude Code Integration | v1.0 | 1/1 | Complete | 2026-04-13 |
| 7. Operational Logging | v1.1 | 1/1 | Complete    | 2026-04-14 |
| 8. Search UX Enhancements | v1.1 | 0/? | Not started | - |
