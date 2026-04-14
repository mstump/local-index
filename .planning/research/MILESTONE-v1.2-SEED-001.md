# Milestone v1.2 — Research notes (SEED-001)

**Purpose:** Condensed input for `/gsd-plan-phase` without re-running full parallel domain research. The authoritative spec is [SEED-001](../seeds/SEED-001-pdf-image-processor-daemon.md).

## Stack direction (Rust-only)

- **Watch + debounce:** `notify` + `notify-debouncer-full` (same family as `local-index` daemon).
- **PDF introspection:** Evaluate `pdf-inspector` vs `lopdf` / `pdf-extract` for classification and text; fall back per SEED if a crate is immature on a target OS.
- **Rasterization:** Platform-specific or `pdfium`/`mupdf` bindings — pick one with a clear license and static-build story for macOS + Linux.
- **HTTP:** `reqwest` for Anthropic Messages and any Google REST/gRPC client if Document AI is enabled.
- **No Python/Node** in the pipeline (project constraint).

## Feature table stakes vs differentiators

| Table stakes | Differentiators |
|--------------|-----------------|
| Companion `.md` output with stable naming | Page-order reassembly with vision described diagrams |
| Idempotent runs via source hash in frontmatter | Optional Google Document AI for OCR-heavy vaults |
| Anthropic vision for semantic image description | — |

## Pitfalls to plan for

- **Double indexing:** Only `.md` companions should contribute chunks; raw PDFs must not duplicate content — enforce naming + README contract (PRE-13).
- **Cost:** Vision + OCR per page can explode API cost — consider caps, batching, and “only if changed” (PRE-04).
- **Classification accuracy:** Misclassified “text” PDFs may yield garbage — allow force-OCR flag in a later iteration if needed (out of v1.2 unless scoped).
- **Credentials:** Google Document AI needs GCP setup — keep optional and document env vars separately from Anthropic.

## Suggested build order

1. Phase 9: walking skeleton + text PDF path + companion format.
2. Phase 10: rasterize + Anthropic OCR + optional Google.
3. Phase 11: vision descriptions + full merge + hash skip polish.
