# Roadmap: local-index

## Milestones

- ✅ **v1.0 Core Indexer** — Phases 1-6 (shipped 2026-04-13)
- ✅ **v1.1 Search UX & Observability** — Phases 7-8 (shipped 2026-04-14)
- ✅ **v1.2 PDF & Image Preprocessor (SEED-001)** — Phases 9-11 (shipped 2026-04-20)

## Phases

<details>
<summary>✅ v1.0 Core Indexer (Phases 1-6) — SHIPPED 2026-04-13</summary>

- [x] **Phase 1: Foundation & File Processing** — CLI skeleton, markdown chunking, config resolution, structured logging (completed 2026-04-09)
- [x] **Phase 2: Storage & Embedding Pipeline** — LanceDB integration, Embedder trait, credential resolution, one-shot indexing (completed 2026-04-10)
- [x] **Phase 3: Search** — Semantic, full-text, and hybrid search with all query flags and output formats (completed 2026-04-10)
- [x] **Phase 4: Daemon Mode & Observability** — File watcher, concurrent runtime, Prometheus metrics, graceful shutdown (completed 2026-04-10)
- [x] **Phase 5: Web Dashboard** — axum-served dashboard with search UI, index browser, status views (completed 2026-04-12)
- [x] **Phase 6: Claude Code Integration** — Skill files, shell wrappers, serve command (completed 2026-04-13)

Archive: `.planning/milestones/v1.0-ROADMAP.md`

</details>

<details>
<summary>✅ v1.1 Search UX & Observability (Phases 7-8) — SHIPPED 2026-04-14</summary>

- [x] **Phase 7: Operational Logging** — Structured search/daemon logging, LanceDB noise suppression (completed 2026-04-14)
- [x] **Phase 8: Search UX Enhancements** — Reranking toggle and query term highlighting in web UI (completed 2026-04-14)

Archive: `.planning/milestones/v1.1-ROADMAP.md`

</details>

<details>
<summary>✅ v1.2 PDF & Image Preprocessor (Phases 9-11) — SHIPPED 2026-04-20</summary>

- [x] **Phase 9: Preprocessor foundation** — Gitignore-aware asset discovery, PDF classification (TextFirst/NeedsVision), local text extraction (lopdf), Anthropic vision client, PDF rasterization, index/daemon wiring, ephemeral cache layout, README docs (completed 2026-04-15)
- [x] **Phase 10: OCR providers** — `OcrService::Anthropic` for scanned PDFs; optional `OcrService::Google` (Document AI) with JWT auth; `--ocr-provider` / `LOCAL_INDEX_OCR_PROVIDER` (completed 2026-04-20)
- [x] **Phase 11: Vision enrichment & idempotency** — SHA-256 cache-read gate (PRE-04); canonical blockquote format `> **[Image: …]** …`; TextFirst PDF per-page text + embedded-image vision interleaving (pdfium-render); graceful degradation; README ephemeral-cache docs (PRE-13) (completed 2026-04-20)

Archive: `.planning/milestones/v1.2-ROADMAP.md`

</details>

## Progress

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1. Foundation & File Processing | v1.0 | 3/3 | Complete | 2026-04-09 |
| 2. Storage & Embedding Pipeline | v1.0 | 3/3 | Complete | 2026-04-10 |
| 3. Search | v1.0 | 2/2 | Complete | 2026-04-10 |
| 4. Daemon Mode & Observability | v1.0 | 3/3 | Complete | 2026-04-10 |
| 5. Web Dashboard | v1.0 | 3/3 | Complete | 2026-04-12 |
| 6. Claude Code Integration | v1.0 | 1/1 | Complete | 2026-04-13 |
| 7. Operational Logging | v1.1 | 1/1 | Complete | 2026-04-14 |
| 8. Search UX Enhancements | v1.1 | 1/1 | Complete | 2026-04-14 |
| 9. Preprocessor foundation | v1.2 | 3/3 | Complete | 2026-04-15 |
| 10. OCR providers | v1.2 | 2/2 | Complete | 2026-04-20 |
| 11. Vision enrichment & idempotency | v1.2 | 3/3 | Complete | 2026-04-20 |
