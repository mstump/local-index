# Phase 9: Preprocessor foundation - Context

**Gathered:** 2026-04-14 (updated 2026-04-14 — discuss-phase refresh)  
**Status:** Ready for planning

<domain>

## Phase Boundary

Deliver the Rust-side pipeline that discovers PDFs and watched image types, classifies PDFs, extracts text **locally** for text-first PDFs, and for **non-text-first PDFs and standalone images** runs **Claude (Anthropic) semantic extraction** so derived text is indexed **before** relying on later-phase refinements (optional OCR providers, blockquote conventions, hash idempotency polish). All of this feeds the **existing indexing path** — with **no permanent converted markdown beside sources in the vault**.

**Roadmap alignment:** This pulls **Anthropic-backed extraction** for scanned/mixed PDFs and for images into Phase 9 from a product perspective. Phases 10–11 should be replanned as **add-ons** (e.g. optional Google Document AI, formatting/idempotency), not as the first time those assets become searchable.

**Canonical source for search and UI:** indexed chunks must be attributed to the **original vault path of the PDF or image**, not to a generated markdown path.

**Ephemeral material:** converted/extracted text may be stored **on disk only under the vault’s `.local-index/`** (or equivalent data dir next to the DB) as a cache for chunking/retry/idempotency — not as Obsidian-visible notes.

**Invocation:** There is **no separate preprocessor subcommand** — PDF/image handling runs **inside `local-index index` and `local-index daemon`** on the same code path that already walks the vault and updates the index.

**Note:** This supersedes the “companion `.processed.md` in the vault” story in SEED-001 / early PRE wording for **how content is surfaced to the user**. README and requirements should be updated during planning so they describe **cache + provenance**, not sidecar markdown in the tree.

</domain>

<decisions>

## Implementation Decisions

### Provenance and paths

- **D-01:** Chunks derived from PDF/image processing use **`file_path` (vault-relative) = path to the source `.pdf` / image file**, not the path of any ephemeral cached text file under `.local-index/`.
- **D-02:** Intermediate text for embedding lives **only under `.local-index/`** (on-disk cache is allowed); it is not a durable vault artifact and must not be what operators edit or what the index browser treats as the “note” path.

### Entry point

- **D-03:** Preprocessing is **folded into `index` and `daemon`** — operators do not run a separate `preprocess` command. One-shot indexing and the file watcher both run the same asset pipeline (discover → extract/cache → chunk/embed) for configured PDF/image types alongside markdown.

### Relationship to older docs

- **D-04:** Treat SEED-001 companion-file naming as **historical** for this milestone until REQUIREMENTS / README are rewritten; the **locked behavior** is ephemeral cache + **asset path in the index** (D-01, D-02).

### Asset types without sufficient local text (Phase 9)

- **D-05:** PDFs classified as **scanned / mixed / not text-first** are still indexed using **Claude semantic extraction** (e.g. vision over rasterized pages or equivalent Messages API usage — exact mechanism is research/planner). They are **not** left unindexed or stub-only in Phase 9.
- **D-06:** **Standalone raster images** (extensions per PRE-02) receive a **Claude-produced description** (semantic extraction) **before** chunks are embedded and stored; Phase 11 may refine format (e.g. blockquotes) and idempotency without deferring “first searchable description” past Phase 9.

### Operator controls

- **D-07:** Operators can **disable PDF+image processing** via **both** a documented **environment variable** and **matching CLI flags** on **`index` and `daemon`** (same precedence pattern as other project config: flag vs env documented in planning).

### Claude's Discretion

- Exact cache file layout, hashing keys, classification crate choice (`pdf-inspector` vs alternatives), and **concrete Anthropic request shape** (batching, page limits, model id) — planner/research within D-01–D-07, subject to **PRE-14** credential patterns.

</decisions>

<canonical_refs>

## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Requirements and roadmap

- `.planning/ROADMAP.md` — Phase 9 scope (may need wording sync with D-01/D-02)
- `.planning/REQUIREMENTS.md` — PRE-* IDs (PRE-13 in particular may need reinterpretation: “no double indexing” via asset path + ignoring raw PDF in walker, not via sidecar naming)

### Seed / research

- `.planning/seeds/SEED-001-pdf-image-processor-daemon.md` — pipeline shape; **vault-local `.processed.md` output is not the chosen v1.2 delivery model** per D-01/D-02
- `.planning/research/MILESTONE-v1.2-SEED-001.md` — stack notes

### Code

- `src/pipeline/store.rs` — `file_path` column; Phase 9+ work must define how PDF/image chunks populate `file_path` as the **asset** path

</canonical_refs>

<code_context>

## Existing Code Insights

### Reusable Assets

- `Chunk` / `ChunkStore` already persist `file_path`; PDF pipeline should set this to the **source asset** path for display, filtering, and delete-by-file consistency.

### Established Patterns

- Vault data under `.local-index/` already used for LanceDB; cache subdir for extracted text fits the same “next to vault” convention.

### Integration Points

- `main` / daemon event handling: extend the existing **index** and **watch→reindex** flows so PDFs/images are processed without a separate CLI surface (D-03).
- Markdown walker today likely skips non-`.md`; integration must **ingest PDF/image-derived text** while keeping **stored `file_path`** as the PDF/image (see D-01).

</code_context>

<specifics>

## Specific Ideas

- User intent: **search hits and paths point at the PDF/image**, not at throwaway converted markdown.

</specifics>

<deferred>

## Deferred Ideas

- Refresh README / PRE-13 text to describe ephemeral cache + asset provenance (planning or doc phase).
- **Reconcile ROADMAP.md Phases 9–11** with D-05/D-06 so Phase 10/11 describe remaining work (optional OCR provider, Google path, blockquote + hash polish) rather than “first OCR” / “first vision.”
- Full **PRE-04** idempotency (hash skip) may still apply to **cache entries** or **content hashes** in metadata — Phase 11 can align with stored hash fields.

### Reviewed Todos (not folded)

- None.

</deferred>

---

*Phase: 09-preprocessor-foundation*  
*Context gathered: 2026-04-14*
