# Phase 10 — Technical Research

**Question:** What do we need to know to plan OCR provider abstraction + optional Google Document AI?

**Status:** Complete

---

## Summary

1. **Refactor boundary:** Today `ingest.rs` calls `AnthropicAssetClient::describe_raster_page` inside the `NeedsVision` branch. Phase 10 introduces a **single dispatch point** (enum or small trait) so the same rasterized PNG buffers feed either Anthropic Messages (image blocks, existing contract) or **Document AI** `processors.process` (raw document with `application/pdf` or page images per API limits).
2. **Anthropic default:** Keep behavior compatible: same user prompt string (`ASSET_VISION_PROMPT`), same model/env vars, unless we add batching later (out of scope unless metrics show necessity).
3. **Google Document AI:** Use the **online** `process` RPC (REST: `POST https://{location}-documentai.googleapis.com/v1/{name}:process` where `name` = `projects/{p}/locations/{loc}/processors/{id}`). Request body: `rawDocument` with base64 content and mime type (`image/png` per page). Response: read `document.text` (full UTF-8) or page blocks — map to markdown lines for `ingest` the same way as Anthropic page strings joined with `\n\n---\n\n`.
4. **Auth:** Follow **PRE-14** — service account JSON via `GOOGLE_APPLICATION_CREDENTIALS` (or ADC), then OAuth2 access token (`Bearer`) for `reqwest`. Rust options: lightweight **JWT bearer** via `jsonwebtoken` + token endpoint, or a small maintained `google-*-auth` helper; avoid Python. Validate token is never logged.
5. **Configuration:** Mirror `LOCAL_INDEX_*` patterns: e.g. `LOCAL_INDEX_OCR_PROVIDER` (`anthropic` | `google`), plus `GOOGLE_CLOUD_PROJECT`, `GOOGLE_DOCUMENT_AI_LOCATION`, `GOOGLE_DOCUMENT_AI_PROCESSOR_ID`. CLI: `--ocr-provider` on `index` and `daemon` with `env =` for each.
6. **Errors:** When `google` is selected, fail with `LocalIndexError::Credential` (or `Config`) listing **all** missing pieces in one message (D-03 in CONTEXT).

---

## Document AI response handling

- Prefer `document.text` when non-empty after `process`.
- If the API returns structured pages only, concatenate `pageLayout` text in page order — must not drop visible text vs today’s Anthropic path.

---

## Risks

| Risk | Mitigation |
|------|------------|
| Token expiry during long PDFs | Refresh token per batch or before loop; reuse `reqwest` client |
| Quota / 429 | Retry with backoff (match `AnthropicAssetClient` style where reasonable) |
| Extra dependencies for JWT | Keep surface minimal; pin versions in `Cargo.toml` |

---

## Validation Architecture

**Dimension 1 (Correctness):** Unit tests on provider dispatch; wiremock for Anthropic unchanged; wiremock for Document AI JSON with synthetic `document.text`.

**Dimension 2 (Regression):** Existing `text_first_pdf_chunks_use_asset_path` and asset tests still pass; NeedsVision with `anthropic` default matches pre–Phase 10 behavior for empty vs non-empty body.

**Dimension 3 (Security):** No API keys or bearer tokens in `tracing::info!` paths; HTTPS only (`googleapis.com` + existing Anthropic base URL).

**Dimension 4 (Config):** Misconfiguration integration test: `ocr-provider=google` with missing env → error string contains required variable names.

**Dimension 8 (Nyquist):** Each plan wave ends with `cargo test` scoped to touched tests; full `cargo test` before verify-work.

---

## RESEARCH COMPLETE
