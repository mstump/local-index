---
phase: 11-vision-enrichment-idempotency
plan: 03
subsystem: docs

tags:
  - documentation
  - readme
  - ephemeral-cache
  - idempotency
  - mermaid
  - pre-13

# Dependency graph
requires:
  - phase: 11-01
    provides: "cache-read gate + corrupt-cache WARN + blockquote_image helper (behavior now documented)"
  - phase: 11-02
    provides: "TextFirst PDF embedded-image vision + `{stem}_page_{N}_image_{I}.png` filename convention (behavior now documented)"

provides:
  - "README.md subsection `### Ephemeral asset cache and idempotency` documenting the SHA-256-keyed ephemeral cache layout (`asset-cache/{shard}/{sha256}.txt`), cache-hit idempotency, corrupt-cache WARN, invalidation command, double-index prevention, TextFirst embedded-image vision, and graceful-degradation fallbacks"
  - "One Mermaid flowchart of `ingest_asset_path` cache gate + dispatch path (per CLAUDE.md Diagrams convention)"

affects:
  - future phases that reindex or extend the asset pipeline (README is now the canonical reference for v1.2 cache semantics)
  - operators evaluating v1.2 behavior (PRE-13 is the last v1.2 milestone deliverable)
  - verifier / milestone closure flow (v1.2 documentation surface now complete)

# Tech tracking
tech-stack:
  added: []  # docs-only plan — no new dependencies, no code change
  patterns:
    - "Mermaid flowchart for pipeline dispatch diagrams (replaces ASCII box-drawing per CLAUDE.md)"
    - "Exact cache-invalidation path in docs (`<data_dir>/asset-cache/` not `<data_dir>/`) to prevent accidental LanceDB destruction (T-11-12 mitigation)"

key-files:
  created: []
  modified:
    - "README.md — inserted `### Ephemeral asset cache and idempotency` subsection (+84 lines) between `## PDF and images (v1.2)` and `### OCR providers (scanned PDFs)`"

key-decisions:
  - "Documentation subsection placed BETWEEN the existing top-level PDF paragraph and the `### OCR providers` heading — does not reword either, preserves verbatim all previously validated v1.1 and earlier content."
  - "Mermaid syntax used for the pipeline dispatch diagram per CLAUDE.md Diagrams convention (`no ASCII box-drawing characters for diagrams in any .md file`). The diagram renders in GitHub, VS Code preview, and any Markdown renderer that supports Mermaid."
  - "Exact cache-invalidation command is `rm -rf <data_dir>/asset-cache/` (with the `asset-cache/` path segment) — deliberately avoids any shorter variant like `rm -rf <data_dir>/` that would destroy the LanceDB index. Mitigates T-11-12 (operator confusion)."
  - "Cache key is source-bytes SHA-256 only — the README explicitly states that `LOCAL_INDEX_ASSET_MODEL` / prompt changes do NOT invalidate cache entries, so operators know they must delete `asset-cache/` after such changes."
  - "Added `text` language hint to the code fence on line 120 (`<data_dir>/asset-cache/ab/cd/{sha256}.txt`) to address the MD040 linter warning; no content change."

patterns-established:
  - "Two-part docs format for pipeline behavior: prose subsection (~350-500 words) + Mermaid flowchart. Applies to future pipeline phases that need doc coverage."
  - "Exact-path invalidation commands in operator docs — always show the full segment (`<data_dir>/asset-cache/`, not `<data_dir>/`) to prevent destructive operator mistakes."
  - "When asserting behavior in docs, grep-bound acceptance criteria (load-bearing strings: `corrupt_cache`, `_page_{N}_image_{I}`, `rm -rf <data_dir>/asset-cache/`) guarantee the doc matches the shipped code."

requirements-completed:
  - PRE-13  # ephemeral-cache documentation + double-index prevention + Phase 11 behavior documented in README

# Metrics
duration: ~5min
completed: 2026-04-20
---

# Phase 11 Plan 03: PRE-13 Ephemeral Cache Documentation Summary

**README now documents the ephemeral `asset-cache/{shard}/{sha256}.txt` layout, cache-hit idempotency, corrupt-cache WARN, cache invalidation procedure, double-index prevention, TextFirst PDF embedded-image vision (`{stem}_page_{N}_image_{I}.png`), and graceful-degradation fallbacks — closing PRE-13 and completing the v1.2 documentation surface.**

## Performance

- **Duration:** ~5 min (single-task docs plan; no RED/GREEN cycle)
- **Started:** 2026-04-20T23:40Z (worktree base reset + README read)
- **Completed:** 2026-04-20T23:45Z (atomic commit)
- **Tasks:** 1 (auto, docs-only)
- **Files modified:** 1 (`README.md`, +84 lines, 0 deletions)

## Accomplishments

- `README.md` gains a new `### Ephemeral asset cache and idempotency` subsection inserted between `## PDF and images (v1.2)` (existing line 101 paragraph) and `### OCR providers (scanned PDFs)` (existing line 111 heading). Every previously validated section (Quick Start, Architecture, Pipeline, Search, CLI Reference, Environment Variables, Claude Code Integration, Building from Source, License) is preserved verbatim.
- Documented the cache layout with the exact sharded path (`asset-cache/ab/cd/{sha256}.txt`) and explained that the two-byte shards are the first four hex characters of the SHA-256 digest — matches the implementation in `src/pipeline/assets/cache.rs::cache_path_for_hash`.
- Documented all three cache behaviors (hit / miss / corrupt) including the `tracing::warn!(corrupt_cache = true, path, error)` structured-log shape from Phase 11-01's `read_cache_if_present` implementation.
- Documented the exact cache-invalidation command (`rm -rf <data_dir>/asset-cache/`) and the source-bytes-SHA-256-only cache-key limitation — operators now know they must delete `asset-cache/` after changing `LOCAL_INDEX_ASSET_MODEL` or the Anthropic vision prompt.
- Documented double-index prevention: markdown walker indexes only `.md` extension; raw `.pdf/.png/.jpg/.jpeg/.webp` files are routed exclusively through the asset pipeline; no companion `.processed.md` files are ever written to the vault.
- Documented the TextFirst PDF embedded-image vision behavior from Phase 11-02: `extract_embedded_images_per_page` + per-image `describe_image` call, with the synthetic filename convention `{stem}_page_{N}_image_{I}.png` (1-based indices) and page separator `\n\n---\n\n`.
- Documented the three-level graceful-degradation contract: (1) missing `ANTHROPIC_API_KEY` on TextFirst PDF with embedded images → single WARN + text-only; (2) missing `libpdfium` → embedded-image extraction skipped with WARN; (3) scanned-PDF rasterization has a separate `pdftoppm` (Poppler) fallback.
- Added a Mermaid flowchart showing `ingest_asset_path`'s cache-gate dispatch — covers the SHA-256 hash, cache-file lookup, WARN path on corrupt-cache, classification + dispatch to TextFirst / NeedsVision / image branches, blockquote composition, and final `tokio::fs::write` cache store. Replaces the project's previous habit of ASCII box-drawing per CLAUDE.md Diagrams convention.

## Task Commits

Single atomic task commit:

1. **Task 1: add Ephemeral asset cache and idempotency subsection to README.md** — `1c11f2e` (docs)

_No REFACTOR or fixup commits needed — the subsection passed every acceptance-criteria grep on the first edit, and `cargo build -p local-index` succeeded unchanged._

## Files Created/Modified

- `README.md` — added `### Ephemeral asset cache and idempotency` subsection (+84 lines). Content order follows the plan exactly: cache layout → cache behavior (hit/miss/corrupt) → cache invalidation → cache-key limitation → double-index prevention → TextFirst embedded-image vision → graceful degradation → Mermaid flowchart. Fenced code block on line 120 annotated with `text` language for MD040 lint compliance; no other stylistic changes.

## Decisions Made

- **Insertion point:** Kept the existing `## PDF and images (v1.2)` paragraph (lines 101-109 of the pre-edit file) unchanged; the new subsection appears as a peer to `### OCR providers (scanned PDFs)` within the v1.2 section. Alternatives (rewording the parent paragraph, replacing the OCR subsection, or adding to a separate "Idempotency" top-level section) would have disturbed previously validated content and broken the existing section-ordering convention.
- **Code-fence language annotation (`text`) on line 120:** The MD040 warning from the IDE diagnostics flagged the unlabeled fence around `<data_dir>/asset-cache/ab/cd/{sha256}.txt`. Setting the language to `text` is the minimal fix — not `sh`, because the content is a literal path not a shell command. This was the only IDE-warning-driven edit; the "Poppler" spell-check error on line 175 is a false positive (Poppler is the correct name of the PDF rendering library from freedesktop.org; the shipped README already contains "Poppler" in other contexts, so ignoring the lint warning preserves consistency).
- **Mermaid over ASCII:** CLAUDE.md's Conventions section explicitly requires Mermaid for all diagrams (`no ASCII box-drawing characters`). The flowchart uses standard Mermaid `flowchart TD` syntax with node shapes (`[...]`, `{...}`, `([...])`) distinguishing process nodes from decision nodes, and `-->` / `-->|label|` edges — renders on GitHub, VS Code preview, and the dashboard's askama templates if ever re-rendered.
- **Word count and tone:** The subsection is ~420 words (plan target 350-500), prose-first with bullet-list cache behaviors and a single diagram. Matches the style of the existing `### OCR providers` subsection (prose + table). No emoji, no marketing language.

## Deviations from Plan

None — plan executed exactly as written.

**Minor post-edit adjustment (not a deviation):** The IDE surfaced an MD040 lint warning on the cache-layout fenced code block (line 120). Added the `text` language identifier to the fence. This is a style cleanup, not a behavior change, and is within the scope of the single `README.md` edit.

**Acceptance-criteria verification:** All 15 grep-based assertions in the plan's `<acceptance_criteria>` block pass:

| Criterion | Expected | Actual |
|-----------|----------|--------|
| `### Ephemeral asset cache and idempotency` | exactly 1 | 1 |
| `asset-cache/ab/cd` | ≥ 1 | 1 |
| `SHA-256` | ≥ 1 | 7 |
| `corrupt_cache` | ≥ 1 | 2 |
| `rm -rf <data_dir>/asset-cache/` | ≥ 1 | 1 |
| `Double-index prevention` | ≥ 1 | 1 |
| `no companion` (case-insensitive) | ≥ 1 | 1 |
| `_page_\{N\}_image_\{I\}` | ≥ 1 | 1 |
| `TextFirst` | ≥ 1 | 4 |
| `Graceful degradation` | ≥ 1 | 1 |
| `pdfium` | ≥ 1 | 3 |
| `` ```mermaid `` | ≥ 1 | 2 (the one added + existing Architecture diagram) |
| `## PDF and images (v1.2)` | exactly 1 | 1 |
| `### OCR providers (scanned PDFs)` | exactly 1 | 1 |
| `ASCII` | 0 OR unchanged | 0 |
| Ordering: new subsection BEFORE OCR providers | awk check passes | passes |
| `cargo build -p local-index` | succeeds | succeeds (1m 41s, clean) |

## Issues Encountered

- **Worktree base drift.** The worktree was initially based on commit `93012096…` (Phase 11-01 completion) rather than the expected `970e73f…` (Phase 11-02 completion + tracking). Resolved via `git reset --hard 970e73f44634e28075bea9f74935f7c91106e7cb` per the `<worktree_branch_check>` protocol. This reset moved the worktree forward to include both Phase 11-01 and Phase 11-02 commits so the Plan 11-03 documentation accurately describes the shipped behavior.
- **IDE spell-check false positive on "Poppler".** The IDE diagnostics flagged "Poppler" on line 175 as a typo (suggesting "Poplar", "Doppler", "Popper"). Poppler is the correct name of the PDF rendering library used as a `pdftoppm` fallback for scanned-PDF rasterization. No change made — the spell-check dictionary is wrong; the shipped documentation is correct.
- **IDE MD040 warning on bare code fence.** Line 120's fenced code block (the `<data_dir>/asset-cache/...` path example) originally had no language hint. Added `text` to satisfy MD040 without changing semantics.
- **No test-suite regressions.** `cargo test -p local-index --lib` runs 118/118 passing identically to Phase 11-02's baseline. README-only edits cannot affect code paths, but the defensive sweep confirms nothing slipped into the commit.

## Next Phase Readiness

- **v1.2 documentation surface complete.** PRE-13 is now the last closed requirement in Phase 11; no further doc-only plans are needed for v1.2. The `/gsd:verify-work` or orchestrator milestone-close flow can mark v1.2 complete once the verifier confirms PRE-13 and the phase-level requirements are all checked off.
- **No blockers.** No code change, no dependency change, no schema change, no public API change. The REQUIREMENTS.md traceability table (updated by the orchestrator) will show PRE-13 checked off against commit `1c11f2e` and Phase 11-03 as the closing plan.
- **Zero new threat surface.** Docs-only change. T-11-11 (information disclosure via README) mitigated: only public env var names (`ANTHROPIC_API_KEY`, `LOCAL_INDEX_DATA_DIR`, `LOCAL_INDEX_ASSET_MODEL`) and public config paths (`asset-cache/{shard}/{sha256}.txt`) appear in the new subsection. T-11-12 (destructive-invalidation mistake) mitigated: the exact path string `<data_dir>/asset-cache/` is present verbatim, and no shorter variant is documented.
- **Future phase coupling:** Any future phase that changes the cache layout, filename convention, or graceful-degradation contract must update the README subsection added in this commit. Load-bearing strings (`asset-cache/ab/cd`, `corrupt_cache`, `_page_{N}_image_{I}`, `rm -rf <data_dir>/asset-cache/`) are documented in this summary's acceptance-criteria table so future verifiers can re-check them.

## Verification Evidence

- `cargo build -p local-index 2>&1 | tail -5` — **succeeds** (1m 41s, `Finished dev profile`)
- `cargo test -p local-index --lib 2>&1 | tail -10` — **118/118 passing** (identical to Phase 11-02 baseline; no regressions)
- Acceptance-grep chain from plan `<verify><automated>` block — **all 7 clauses pass** (`README-PRE-13-OK`)
- Full 15-criterion acceptance table — **all pass** (see Deviations table above)
- Ordering check (`awk` on section line numbers) — **passes** (`Ephemeral asset cache` at line 111, `OCR providers` at line 201, so new subsection precedes existing one)
- Git post-commit deletion check — **zero deletions** (only insertions)

## Self-Check: PASSED

Verified file exists at expected path:

- FOUND: `README.md` (contains `### Ephemeral asset cache and idempotency`, `asset-cache/ab/cd`, `corrupt_cache`, `_page_{N}_image_{I}`, `rm -rf <data_dir>/asset-cache/`, and one ` ```mermaid ` flowchart fence introduced in this plan)

Verified commit exists:

- FOUND: `1c11f2e` (docs Task 1: add Ephemeral asset cache and idempotency subsection)

No known stubs introduced — the documentation describes behavior that is already shipped in Phase 11-01 (`read_cache_if_present`, blockquote_image) and Phase 11-02 (`extract_embedded_images_per_page`, TextFirst per-page loop). No threat flags beyond the two documented in the plan's `<threat_model>` (T-11-11 and T-11-12, both mitigated as specified).

---

*Phase: 11-vision-enrichment-idempotency*
*Plan: 03*
*Completed: 2026-04-20*
