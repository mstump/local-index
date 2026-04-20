# Phase 11: Vision enrichment & idempotency - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-20
**Phase:** 11-vision-enrichment-idempotency
**Areas discussed:** Companion files vs ephemeral, Idempotency mechanism, Blockquote format scope, Mixed PDF interleaving depth

---

## Companion files vs ephemeral

| Option | Description | Selected |
|--------|-------------|----------|
| Stay ephemeral | Keep Phase 9's decision: image descriptions stay in in-memory synthetic markdown → ephemeral cache under .local-index/. No new files appear in the vault. | ✓ |
| Reintroduce companion files | Revive SEED-001: write .processed.md files into the vault alongside originals. Requires revisiting D-01, walker updates, file management. | |

**User's choice:** Stay ephemeral
**Notes:** Phase 9 D-01/D-02/D-04 preserved. No companion files in vault.

---

## Idempotency mechanism

| Option | Description | Selected |
|--------|-------------|----------|
| Cache hit skips API call | Compute source SHA-256 first. If asset-cache/{shard}/{sha256}.txt exists, read synthetic markdown from cache — no Anthropic/OCR API call. | ✓ |
| LanceDB metadata check first | Query store for chunk with file_path + stored source-hash field. Requires adding source_hash column to LanceDB schema. | |
| Both layers | Cache hit → skip API; LanceDB source_hash → skip chunk comparison. Maximum coverage but adds schema migration. | |

**User's choice:** Cache hit skips API call

| Failure mode | Description | Selected |
|---|---|---|
| Re-fetch silently | Treat any cache read error or empty file as cache miss. | |
| Log warning + re-fetch | Log WARN line and call API. | ✓ |
| Fail the asset | Return error when cache file is unreadable. | |

**User's choice:** Log warning + re-fetch
**Notes:** Corrupt cache logs WARN and re-fetches. No schema migration needed.

---

## Blockquote format scope

| Option | Description | Selected |
|--------|-------------|----------|
| Both standalone + PDF pages | Apply blockquote to ALL image descriptions. | ✓ |
| PDF NeedsVision pages only | Only rasterized PDF pages get blockquote. | |
| Standalone images only | Only standalone image descriptions use blockquote. | |

**User's choice:** Both standalone + PDF pages

| Label | Description | Selected |
|-------|-------------|----------|
| Filename only | `> **[Image: figure_1.png]** desc` — matches SEED-001 example. | ✓ |
| Vault-relative path | Full path for disambiguation. | |
| Page number for PDFs | `> **[Image: page 3 of report.pdf]** ...` | |

**User's choice:** Filename only

| Separator | Description | Selected |
|-----------|-------------|----------|
| --- horizontal rule | Existing behavior — pages separated by markdown horizontal rule. | ✓ |
| ## Page N heading | Each page becomes a heading section with breadcrumbs. | |
| No separator | Pages joined directly. | |

**User's choice:** --- (existing behavior preserved)

---

## Mixed PDF interleaving depth

| Option | Description | Selected |
|--------|-------------|----------|
| NeedsVision page order only | NeedsVision pages already in order; apply blockquote + ---. TextFirst stays text-only. Minimal scope. | |
| TextFirst gets embedded images too | Native pdfium image extraction from TextFirst pages; vision on pages with ≥1 image. Richer but significant scope. | ✓ |

**User's choice:** TextFirst gets embedded images too

| Extraction method | Description | Selected |
|---|---|---|
| Rasterize TextFirst pages too | Reuse pdf_raster.rs; rasterize each page, call vision on image-heavy pages. | |
| Extract embedded images natively | Pull embedded image objects from PDF without rasterizing. No new crate needed if pdfium exposes it. | ✓ |
| Claude decides | Leave to planner/researcher. | |

**User's choice:** Extract embedded images natively

| Heuristic | Description | Selected |
|-----------|-------------|----------|
| All pages with ≥1 extracted image | Call vision on every page that yields ≥1 embedded image object. | ✓ |
| Images above size threshold | Only call vision on images larger than configurable minimum dimension. | |
| Claude decides threshold | Leave filtering to planner. | |

**User's choice:** All pages with ≥1 extracted image

---

## Claude's Discretion

- How to interleave extracted text paragraphs and image blockquotes within a page (before/after/adjacent to image position)
- Whether pdfium image extraction returns positional metadata
- Exact WARN log field names for corrupt cache

## Deferred Ideas

- Size/area threshold for embedded image filtering (logos, decorative graphics)
- LanceDB `source_hash` column for source-level idempotency
- TextFirst PDF rasterization as fallback if native extraction is insufficient
