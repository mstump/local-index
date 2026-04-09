---
id: SEED-001
status: dormant
planted: 2026-04-08
planted_during: v1 initialization (pre-Phase 1)
trigger_when: when a PDF support phase is being planned for local-index
scope: medium
---

# SEED-001: PDF & Image Processor Daemon

## Why This Matters

local-index v1 only indexes markdown files. Real Obsidian vaults contain PDFs (papers,
receipts, notes scanned from paper) and images (diagrams, screenshots, whiteboards).
Without processing these, a significant fraction of the vault is invisible to semantic
search. This companion daemon fills that gap by converting PDFs and images into enriched
markdown that local-index can index like any other `.md` file — no changes required to
the core indexer.

The architecture is clean: this daemon is a *pre-processor*, not a replacement. It
produces `.processed.md` companion files that local-index picks up automatically.

## When to Surface

**Trigger:** When a phase explicitly targeting PDF/image support is being planned.

This seed should be presented during `/gsd:new-milestone` when the milestone scope
matches any of these conditions:
- New milestone includes "PDF support", "document formats", or "image ingestion"
- v1 is shipped and a v2 milestone is being scoped
- `FMT-01` or `FMT-02` from REQUIREMENTS.md are being promoted from v2 → v1

## Architecture (captured at seed time)

```
filesystem watcher (notify crate, debounced 500ms)
    │
    ▼
classification (pdf-inspector crate)
    │
    ├─ text-based PDF  → local text extraction + image extraction
    ├─ scanned/mixed PDF → OCR provider (configurable: anthropic | google-docai)
    ├─ image file (.png, .jpg, .webp) → skip to semantic enrichment
    │
    ▼
image semantic extraction (Anthropic Messages API, vision)
    │
    ▼
reassemble: interleave text + image descriptions in page order
    │
    ▼
write .processed.md companion file with YAML frontmatter (content hash stored)
```

## Core Requirements (captured at seed time)

**Filesystem watcher:**
- `notify` crate, 500ms debounce, watches for `*.pdf`, `*.png`, `*.jpg`, `*.jpeg`, `*.webp`
- Skip files where a `.processed.md` companion already exists with a matching SHA-256 content hash in frontmatter
- Respects `.gitignore` and configurable exclude list

**PDF classification & text extraction:**
- `pdf-inspector` crate classifies each PDF as `TextBased`, `Scanned`, `ImageBased`, or `Mixed`
- `TextBased` pages: extract text locally, convert to markdown
- `Scanned`/`ImageBased`/`Mixed`: rasterize to PNG → OCR provider

**OCR provider (configurable):**
- **Anthropic** (default): rasterize → send as `type: "image"` to Messages API (`claude-sonnet-4-*`), prompt for structured markdown output
- **Google Document AI**: send via gRPC (`tonic`), convert structured `Document` response to markdown

**Image semantic extraction:**
- Every extracted image (from PDFs) and every standalone image file
- Anthropic Messages API vision, regardless of OCR provider setting
- Prompt: describe semantic content — what it depicts, visible text, data points (charts/diagrams), relationships/entities
- Always uses Anthropic for this step

**Document reassembly:**
- Single markdown file per PDF, interleaving text and image descriptions in page order
- Image descriptions formatted as blockquotes:
  ```
  > **[Image: figure_1.png]** Description of the semantic content
  > extracted by the vision model.
  ```
- Standalone images produce a short markdown file with the description as the body

**Output:** `.processed.md` companion file with YAML frontmatter including content hash

## Implementation Notes

- Can be a separate binary in the same workspace (`local-index-processor`) or a subcommand
  of `local-index` (`local-index process`)
- Reuses the same credential resolution logic as local-index (ANTHROPIC_API_KEY / ~/.claude/)
- The `.processed.md` companion file naming convention must be agreed upon so local-index
  doesn't double-index both the original and the companion
- `pdf-inspector` crate needs evaluation — may need `lopdf` as fallback for image extraction
- Google Document AI requires a GCP project and service account; adds a new credential type

## Scope Estimate

**Medium** — 2-3 phases. The pipeline is well-defined but has several moving parts:
- Phase A: watcher + PDF classification + local text extraction + companion file output
- Phase B: OCR provider integration (Anthropic vision + Google Document AI)
- Phase C: image semantic extraction + full reassembly + hash-based skip logic

## Breadcrumbs

Related decisions and requirements in the current project:

- `.planning/PROJECT.md` — "PDF support — deferred to v2" in Out of Scope
- `.planning/REQUIREMENTS.md` — `FMT-01` (PDF), `FMT-02` (DOCX) in v2 Requirements
- `.planning/research/FEATURES.md` — Anti-features: "PDF / DOCX / image ingestion" section explains the deferral rationale
- `.planning/research/PITFALLS.md` — may contain notes on extraction pipeline complexity

## Notes

The user provided a detailed architecture description (April 2026) during v1 initialization.
The design was intentionally deferred because it's a companion pre-processor, not core to
the v1 markdown indexing pipeline. The full spec is preserved here so no detail is lost
when v2 scope is being defined.
