# Project Retrospective

*A living document updated after each milestone. Lessons feed forward into future planning.*

---

## Milestone: v1.2 — PDF & Image Preprocessor

**Shipped:** 2026-04-20
**Phases:** 3 (9–11) | **Plans:** 8 | **Duration:** ~5 days (2026-04-15 → 2026-04-20)

### What Was Built

- Gitignore-aware asset discovery + PDF classification (TextFirst/NeedsVision) + local lopdf text extraction
- Anthropic vision client (`AnthropicAssetClient`) for standalone images and scanned PDFs; PDF rasterization via pdfium-render with pdftoppm fallback
- `OcrService` enum dispatch with `OcrService::Anthropic` (default) and `OcrService::Google` (Document AI); JWT service-account auth; `--ocr-provider` / `LOCAL_INDEX_OCR_PROVIDER`
- SHA-256 ephemeral cache (`asset-cache/{shard}/{sha256}.txt`) — cache-read gate short-circuits all OCR/vision on unchanged sources
- Canonical blockquote format `> **[Image: {filename}]** {desc}` for all vision output
- TextFirst PDF per-page text + embedded-image interleaving via `extract_embedded_images_per_page` (pdfium-render)
- Graceful degradation at 3 levels: no pdfium, no `ANTHROPIC_API_KEY`, per-image vision failure
- README documentation of ephemeral cache layout, invalidation, double-index prevention, graceful-degradation contract

### What Worked

- **TDD discipline paid off.** Plans 11-01 and 11-02 each followed strict RED → GREEN → (no REFACTOR needed) cycles. Reference implementations in the plan translated verbatim on first try; every acceptance criterion passed on first GREEN run.
- **Small, focused plans.** Plan 11-01 was 12 min, Plan 11-03 was 5 min. Tight scope = tight execution. Phase 9 plans averaged ~45 min each.
- **Wiremock contract tests.** Having Anthropic and Voyage mocked at the HTTP layer caught real integration issues (e.g., worktree base drift on OCR providers, pin requirement for `image` crate) without needing live API credentials.
- **`gsd-tools milestone complete` CLI.** Automated archive creation, MILESTONES.md update, and STATE.md update — no manual bookkeeping.
- **CLAUDE.md Mermaid convention enforced.** Plan 11-03 added a Mermaid flowchart to README without discussion; the convention was clear from CLAUDE.md.

### What Was Inefficient

- **Worktree base drift (3 times).** Three separate plans (11-01, 11-02, 11-03) started on wrong base commits and required `git reset --hard` before edits could begin. The `<worktree_branch_check>` protocol caught this, but the reset added friction each time. Root cause: worktrees were created before Phase 11 plans landed in main.
- **Pre-existing clippy errors block `cargo clippy --all-targets -D warnings`.** ~75 errors in `claude_rerank.rs`, `search/types.rs`, `pipeline/store.rs` etc. are out-of-scope per deviation rules but force all plans to run `cargo clippy --lib` on touched files only. These should be cleaned up before v1.3 starts.
- **`image` crate pin needed immediate Cargo.toml change.** The 0.25.10 → 0.25.4 pin was not caught in the research phase; it added an unplanned Cargo.toml change mid-Phase 9.

### Patterns Established

- **Cache-read gate before per-extension branching.** Hash source bytes once at the top of `ingest_asset_path`; all downstream API branches are guarded. Future asset pipeline phases should reuse this pattern, not add their own caches.
- **`pub mod test_support` for cross-crate fixture sharing.** Integration tests in `tests/` need `pub fn` access to fixtures that live inside `src/`. The `src/test_support.rs` re-export shim avoids `#[cfg(test)]` feature-flag gymnastics.
- **Runtime pdfium probe over `#[cfg(feature)]` gates.** `Pdfium::bind_to_system_library().is_ok()` in tests keeps CI green on Poppler-only hosts without requiring conditional feature flags. The probe is cheap (~1 ms).
- **`ingest_asset_path(pdf_ocr, image_vision)` parameter split.** Separating PDF OCR from standalone image vision in the function signature enables clean `OcrService` dispatch while keeping images Anthropic-only.
- **Exact-path invalidation commands in docs.** Always show full segment (`<data_dir>/asset-cache/`) to prevent accidental destruction of the LanceDB index.

### Key Lessons

1. **Plan for graceful degradation explicitly.** Plans 11-01 and 11-02 both had explicit `<threat_model>` rows for pdfium-missing, ANTHROPIC_API_KEY-missing, and per-image failure. This made the 3-level degradation contract obvious and prevented the common trap of only writing happy-path code.
2. **Accepted risks belong in plan frontmatter, not post-hoc comments.** T-11-07 (information disclosure to Anthropic) was marked as accepted risk in the plan and surfaced to documentation in Plan 11-03. This is cleaner than discovering it during review.
3. **Worktree creation timing matters.** Create worktrees from the phase head commit *after* all preceding plans have been merged to main. Creating them early leads to base drift that must be resolved before work begins.
4. **Wiremock + `CARGO_BIN_EXE_local-index` for integration tests.** Using the cargo-provided binary path (not hardcoded `target/debug/local-index`) prevents stale binary issues in integration tests. This pattern is now established in `tests/index_assets_integration.rs`.

### Cost Observations

- Model mix: quality profile (Sonnet 4.x throughout)
- Sessions: ~5–6 focused sessions across phases 9–11
- Notable: Plans 11-01 (12 min) and 11-03 (5 min) demonstrate that tight, well-researched plans execute near-instantly. Plan 11-02 (~40 min) needed a runtime pdfium-availability probe fix — the only deviation that required investigation.

---

## Cross-Milestone Trends

### Process Evolution

| Milestone | Phases | Plans | Key Process Change |
|-----------|--------|-------|--------------------|
| v1.0 | 6 | 15 | Initial GSD setup; established core patterns |
| v1.1 | 2 | 2 | Tightened scope; shorter milestones work well |
| v1.2 | 3 | 8 | TDD with wiremock; pdfium graceful-degradation probes |

### Cumulative Quality

| Milestone | Lib Tests | Integration Tests | New Dependencies Added |
|-----------|-----------|-------------------|----------------------|
| v1.0 | ~80 | search_integration | clap, lancedb, tantivy, reqwest, axum, notify |
| v1.1 | ~95 | search_ux | (none) |
| v1.2 | 118 | anthropic_assets_mock, index_assets_integration, document_ai_mock | pdfium-render, lopdf, base64, image, jsonwebtoken, ignore |

### Top Lessons (Verified Across Milestones)

1. **Tight plan scope = fast execution.** v1.1 (2 plans, shipped in 1 day) and Plans 11-01/11-03 confirm that single-focus plans with explicit acceptance criteria execute reliably. Multi-concern plans increase rework risk.
2. **Graceful degradation > hard failures.** Every phase that touched optional external tools (pdfium, Anthropic, Google) invested in fallback paths. None shipped with hard panics on missing dependencies.
3. **Wiremock first, live API never.** All three milestones avoided live API calls in tests. Wiremock contracts caught integration issues early and keep CI hermetic.
