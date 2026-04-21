# Milestones: local-index

## v1.2 PDF & Image Preprocessor (Shipped: 2026-04-21)

**Phases completed:** 3 phases, 8 plans, 17 tasks

**Key accomplishments:**

- Established the preprocessor asset pipeline building blocks: gitignore-aware discovery, local PDF text extraction with a documented density heuristic, and sharded cache paths — ready for Anthropic wiring in Plan 02.
- Introduced `OcrService::Anthropic`, split PDF OCR from standalone image vision in `ingest_asset_path`, and wired CLI plus daemon so scanned PDFs use the OCR enum while images still call `describe_image`.
- Delivered optional Google Document AI for scanned PDF OCR via `OcrService::Google`, JWT-based service-account auth, `--ocr-provider` / `LOCAL_INDEX_OCR_PROVIDER`, wiremock integration test, and README coverage.
- Cache-read gate above per-extension branching in `ingest_asset_path` plus canonical `>
- TextFirst PDFs now extract embedded raster images per page (pdfium-render + `get_raw_image()`) and send each one through `AnthropicAssetClient::describe_image`, interleaving the per-page text with one `>
- README now documents the ephemeral `asset-cache/{shard}/{sha256}.txt` layout, cache-hit idempotency, corrupt-cache WARN, cache invalidation procedure, double-index prevention, TextFirst PDF embedded-image vision (`{stem}_page_{N}_image_{I}.png`), and graceful-degradation fallbacks — closing PRE-13 and completing the v1.2 documentation surface.

---

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

## v1.1 — Search UX & Observability (Complete)

**Completed:** 2026-04-14
**Phases:** 7–8

**What shipped:**

- Phase 7: Operational logging — search queries and daemon file events at INFO; LanceDB noise suppression
- Phase 8: Search UX — web reranking toggle, query-term highlighting in snippets

**Archive:** `.planning/milestones/v1.1-phases/`

---
