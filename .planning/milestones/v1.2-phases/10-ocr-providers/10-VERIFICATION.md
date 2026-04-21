---
status: passed
phase: 10-ocr-providers
completed: 2026-04-20
---

# Phase 10 verification — OCR providers

## Goal (from ROADMAP)

Rasterize scanned PDF pages; Anthropic Messages OCR path; optional Google Document AI when configured (**PRE-07**, **PRE-08**).

## Must-haves

| Requirement | Evidence |
|-------------|----------|
| NeedsVision uses pluggable OCR (not direct `describe_raster_page` in `ingest`) | `ingest.rs` calls `pdf_ocr.ocr_scanned_pdf_pages`; `OcrService` enum in `ocr.rs` |
| Default Anthropic path preserves Phase 9 behavior | Wiremock tests + existing asset integration tests pass |
| Google provider opt-in via env + CLI | `OcrProvider`, `--ocr-provider`, `LOCAL_INDEX_OCR_PROVIDER`; `cli.rs` + `resolve_ocr_provider` |
| Google misconfiguration lists missing keys | `validate_google_document_ai_config`, unit test `google_ocr_validation_lists_missing_keys` |
| README documents switching | README section **OCR providers (scanned PDFs)** |
| Automated tests | `cargo test -p local-index` green; `tests/document_ai_mock.rs` |

## Human verification

Optional: run `local-index index` with `LOCAL_INDEX_OCR_PROVIDER=google` and real GCP credentials against a non-production processor — not required for automated verification.

## Conclusion

Phase goal met: Anthropic default OCR abstraction plus optional Google Document AI with documented env vars and failing-fast validation.
