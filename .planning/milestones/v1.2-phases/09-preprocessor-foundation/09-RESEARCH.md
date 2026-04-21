# Phase 9: Preprocessor foundation — Technical research

**Status:** Complete  
**Date:** 2026-04-14

---

## Scope alignment (locked from 09-CONTEXT.md)

- **No** Obsidian-visible companion `.md` next to PDFs/images. Derived text lives under the vault’s data dir (e.g. `<vault>/.local-index/...` or `LOCAL_INDEX_DATA_DIR`), not in the note tree.
- **Provenance:** Every stored chunk’s `file_path` is the **vault-relative path to the source asset** (`.pdf`, `.png`, …), not the cache file path.
- **Entry points:** Same pipeline for `local-index index` and daemon watch path — **no** separate `preprocess` subcommand (overrides older PRE-01 “subcommand” wording: satisfaction = Rust code in this workspace invoked from `index`/`daemon`).
- **Phase 9 must index:** text-first PDFs via **local** extraction; **non-text-first PDFs** and **standalone images** via **Anthropic** (Messages API with image parts — vision / semantic description). Phase 10+ adds alternate OCR providers and formatting polish.

---

## Standard stack

| Concern | Recommendation | Notes |
|--------|------------------|-------|
| Ignore rules | `ignore` crate (`WalkBuilder`) | Same engine as ripgrep; respects `.gitignore` + optional extra overrides |
| PDF structure / text | `lopdf` + heuristics, or dedicated inspector crate if evaluation shows better classification | Extract text per page where possible; count chars vs pages for “text-first” signal |
| PDF rasterization (scanned / mixed) | `pdfium-render` **or** system `pdftoppm` via `std::process::Command` (document platform deps) | Rust-native PDFium binding is common for cross-platform; subprocess acceptable if documented for macOS/Linux |
| Images | `image` crate | Decode PNG/JPEG/WebP for base64/API payloads |
| HTTP to Anthropic | Existing `reqwest` + JSON patterns from `src/claude_rerank.rs` | Reuse API version header, key from `ANTHROPIC_API_KEY`, clear errors when missing for asset path |
| Async | `tokio` | Long I/O and HTTP off main thread; batch embeddings already async |
| Debounce | Existing `notify-debouncer-full` in daemon | Extend event filter for asset extensions alongside `.md` |

---

## Integration points (current codebase)

- **Discovery:** `src/pipeline/walker.rs` — today only `*.md`. Add parallel discovery (or unified walk) for configured asset extensions without indexing raw bytes as markdown.
- **Index loop:** `src/main.rs` — after markdown pass (or interleaved), run asset pipeline → synthetic markdown string → `chunk_markdown` / same embed path with `Chunk.file_path = PathBuf` of **asset**.
- **Daemon:** `src/daemon/processor.rs` — today gates on `extension == md`. Add branches for asset extensions: same `reindex_file`-style flow but content from pipeline.
- **Deletes/renames:** `ChunkStore::delete_chunks_for_file` keyed by vault-relative string — must use **asset** path consistently.
- **Credentials:** `ANTHROPIC_API_KEY` mirrors rerank; PRE-14 = extend `credentials` module or shared helper with actionable errors (optional `~/.claude/` only if project explicitly adopts — today rerank uses env only).

---

## Risks and mitigations

| Risk | Mitigation |
|------|------------|
| API cost explosion (vision per page / image) | Configurable max pages per PDF per run; skip or warn over limit; reuse cache when source SHA-256 unchanged (initial cache metadata in Phase 9; full hash-idempotent story can align with Phase 11) |
| PDFium / native deps on Linux | Feature flag or document `LOCAL_INDEX_PDF_RENDER=...`; CI uses unit tests with mocked extractors where possible |
| Double indexing | Do not emit separate index rows for cache paths; never set `file_path` to cache file |
| Large binary read DoS | Cap read size (configurable), log skip |
| Licensing (PDFium) | Document in README |

---

## Requirements reinterpretation

- **PRE-01:** Satisfied by extending the **`local-index`** binary (`index` / `daemon`) in this repo — not a separate Node/Python tool. Subcommand not required per CONTEXT D-03.
- **PRE-02:** Daemon debounced events must include asset extensions (in addition to `.md`).
- **PRE-03:** `.gitignore` via `ignore` crate + CLI/env exclude list (comma-separated globs or repeated flag — planner chooses one and documents).
- **PRE-13 (initial):** “No double indexing” = chunks attributed to **asset** path only; README describes ignoring raw PDF in “snippet source” is wrong — hits show PDF path; operators may still want to exclude paths from the vault walk via ignore patterns.

---

## Validation architecture

Phase verification is **Rust-first**:

- **Unit tests:** PDF classification helpers, gitignore walker filtering, markdown synthesis from extracted text (no network).
- **Integration tests:** Temp vault + tiny synthetic PDF (or fixture checked into `tests/fixtures/`) for local extract path; Anthropic paths behind `#[ignore]` or `wiremock` HTTP mock matching `src/claude_rerank.rs` test style.
- **Commands:** `cargo test`, `cargo test --test <name>`, `cargo clippy -- -D warnings` (as project already uses).

Downstream `/gsd-execute-phase` should run `cargo test` after each plan wave and fix regressions before closing the plan.

---

## Open decisions for implementation (planner resolved)

1. Exact PDF “text-first” threshold (e.g. min chars per page, pdf-inspector vs lopdf).
2. Rasterization crate vs subprocess — pick one in Plan 09-01/09-02 with CI impact noted.
3. Cache file naming: `{sha256}.md` or `{relative_path_sanitized}.cache` under `asset_cache/` subtree.

## RESEARCH COMPLETE
