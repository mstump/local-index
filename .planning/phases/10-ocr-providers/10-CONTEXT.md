# Phase 10: OCR providers - Context

**Gathered:** 2026-04-16  
**Status:** Ready for planning

<domain>

## Phase Boundary

Phase 10 delivers **PRE-07** and **PRE-08**: scanned / image-heavy PDF pages continue to be **rasterized**, then text is obtained through a **pluggable OCR path** whose **default** is the existing **Anthropic Messages API** image flow (`AnthropicAssetClient::describe_raster_page` and call sites in `ingest.rs`). **Google Document AI** is added as an **optional** provider when operator config and credentials are present.

**In scope:** Provider abstraction in Rust (no new subprocesses beyond what rasterization already uses), configuration surface (env + CLI flags consistent with `index` / `daemon`), clear failure modes when a provider is selected but misconfigured, and documentation so operators can switch Anthropic vs Google safely.

**Explicitly out of scope (later phases):** PRE-09–PRE-12 (full vision enrichment polish, blockquote convention, single-file reassembly, standalone-image companions, hash idempotency) remain **Phase 11** per ROADMAP. Phase 10 may **not** require changing chunk `file_path` semantics (locked in Phase 9 `09-CONTEXT.md` D-01).

**Relationship to Phase 9:** Rasterization (`src/pipeline/assets/pdf_raster.rs`) and per-page Anthropic vision already exist for `PdfClassification::NeedsVision`. Phase 10 **refines and generalizes** that path into an OCR provider model and adds Document AI — it does **not** re-litigate ephemeral cache or provenance decisions.

</domain>

<decisions>

## Implementation Decisions

### Provider model

- **D-01:** Introduce an **OCR provider** abstraction used only for **rasterized PDF pages** on the NeedsVision path (and any future shared hook standalone code might reuse later without implementing PRE-09 here). **Anthropic** and **Google Document AI** are the two implementations for this phase.
- **D-02:** **Anthropic** remains the **default** when no override is set; behavior stays compatible with today’s per-page vision calls unless research shows a single batched call is strictly better (Claude’s discretion in planning).
- **D-03:** When **Google Document AI** is selected, processing **fails fast with an actionable message** listing required settings (project, location, processor id, credential source) if any are missing — mirror the clarity of `ANTHROPIC_API_KEY` errors on the vision path.

### Configuration

- **D-04:** Provider choice is exposed via **environment variable and matching CLI flags** on **`index` and `daemon`**, following the same precedence pattern as other project config (document in README alongside existing asset flags).
- **D-05:** Google path is **opt-in only**; default installs and docs assume Anthropic only. No Google credentials are required for the default configuration.

### Output contract

- **D-06:** Regardless of provider, the pipeline must produce **markdown body text** that feeds the **existing** heading-based chunking and embedding path (same shape as today’s synthetic markdown from `ingest.rs`). Provider-specific structured responses must be **mapped to markdown** in Rust.

### Claude's Discretion

- Document AI transport (REST vs gRPC), batching vs per-page calls, retry/backoff, and exact env names — planner/researcher within D-01–D-06 and **PRE-14** credential patterns.
- Whether to add a minimal **trait** (`OcrProvider`) vs an enum dispatch in one module — follow existing codebase style (`Embedder`-like vs small enums).

</decisions>

<canonical_refs>

## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Requirements and roadmap

- `.planning/ROADMAP.md` — Phase 10 goal, success criteria, PRE-07 / PRE-08
- `.planning/REQUIREMENTS.md` — PRE-07, PRE-08, PRE-14 traceability

### Prior phase context

- `.planning/phases/09-preprocessor-foundation/09-CONTEXT.md` — provenance, cache, entry points, D-05/D-06 scope split with Phase 10+
- `.planning/phases/09-preprocessor-foundation/09-RESEARCH.md` — rasterization and API patterns

### Seed / research

- `.planning/seeds/SEED-001-pdf-image-processor-daemon.md` — historical Document AI note (output model differs from shipped cache design; follow Phase 9 D-01/D-02 for storage, not vault-local companions)

### Code (integration points)

- `src/pipeline/assets/ingest.rs` — `NeedsVision` branch; primary insertion point for provider dispatch
- `src/pipeline/assets/anthropic_extract.rs` — default Anthropic implementation to wrap or call from trait
- `src/pipeline/assets/pdf_raster.rs` — rasterization inputs to OCR providers
- `src/credentials.rs` — pattern for resolving API keys
- `src/main.rs`, `src/daemon/mod.rs` — wiring new flags/env into asset client / ingest

</canonical_refs>

<code_context>

## Existing Code Insights

### Reusable Assets

- `AnthropicAssetClient` and `describe_raster_page` already implement the default OCR-by-vision path; Phase 10 should **refactor behind a provider interface** rather than duplicating HTTP logic.
- `rasterize_pdf_pages_to_png` already yields per-page PNG buffers suitable for both Anthropic image parts and typical Document AI **process** requests.

### Established Patterns

- `LocalIndexError::Credential` / `Config` for operator-facing errors with fix hints.
- Optional integrations constructed at startup in `main` and daemon (`try_from_env` style).

### Integration Points

- `ingest_asset_path` and PDF branch in `ingest.rs` — inject provider or factory from `AppState` / daemon processor alongside existing `AnthropicAssetClient` option.

</code_context>

<specifics>

## Specific Ideas

- Success criteria: a scanned PDF still produces **non-empty** markdown suitable for search after OCR; switching providers is **documented** and misconfiguration **fails clearly** (ROADMAP Phase 10 success criteria).

</specifics>

<deferred>

## Deferred Ideas

- **PRE-09 – PRE-12**, **PRE-04** hash skip, blockquote image format, full page-order reassembly — **Phase 11** per ROADMAP.
- Updating REQUIREMENTS traceability table rows that still say “Pending” for items Phase 9 already implemented — backlog / doc pass, not blocking Phase 10 planning.

### Reviewed Todos (not folded)

- None.

</deferred>

---

*Phase: 10-ocr-providers*  
*Context gathered: 2026-04-16*
