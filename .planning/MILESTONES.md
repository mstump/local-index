# Milestones: local-index

## v1.0 — Core Indexer (Complete)

**Completed:** 2026-04-13
**Phases:** 1–6

**What shipped:**

- Phase 1: CLI skeleton, markdown chunker with smart size-based splitting, YAML frontmatter parsing, structured logging
- Phase 2: Voyage AI embeddings (voyage-3.5), LanceDB storage with Arrow schema, SHA-256 incremental indexing
- Phase 3: Semantic/FTS/hybrid search (RRF k=60), path/tag filters, JSON/pretty output, integration tests
- Phase 4: File watcher daemon (notify + debouncer), Prometheus metrics, HDR histograms, graceful shutdown
- Phase 5: axum web dashboard — search UI, index browser, status, embedding stats, settings
- Phase 6: Claude Code skill files (search, reindex, status), shell wrappers, README integration docs

**Archive:** `.planning/milestones/v1.0-phases/`

---
