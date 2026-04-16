---
phase: 09-preprocessor-foundation
plan: 02
subsystem: assets-vision
tags: [rust, anthropic, voyage, pdfium, wiremock]

requires:
  - phase: 09-01
    provides: Asset discovery, PDF classification, cache paths
provides:
  - Anthropic Messages client for image / raster-page descriptions (`AnthropicAssetClient`)
  - PDF rasterization via PDFium with `pdftoppm` fallback
  - `LOCAL_INDEX_ANTHROPIC_BASE_URL` / `new_for_test` for wiremock
  - `LOCAL_INDEX_VOYAGE_BASE_URL` read in `VoyageEmbedder::new` for integration tests
affects:
  - "09-03 (index/daemon ingest wiring)"

tech-stack:
  added: ["pdfium-render", "base64", "image (=0.25.4)"]
  patterns:
    - "Credential errors for missing `ANTHROPIC_API_KEY` on vision-required paths (PRE-14)"
    - "Wiremock contract on `ASSET_VISION_PROMPT` + base64 image payload"

key-files:
  created:
    - src/pipeline/assets/anthropic_extract.rs
    - src/pipeline/assets/pdf_raster.rs
    - tests/anthropic_assets_mock.rs
  modified:
    - Cargo.toml / Cargo.lock
    - src/credentials.rs
    - src/error.rs
    - src/pipeline/embedder.rs
    - src/pipeline/assets/pdf_local.rs

key-decisions:
  - "Pin `image` to 0.25.4: 0.25.10 pulled `png` 0.18 pre-release and broke `cargo build`"
  - "`fixture_single_page_text_pdf` is `#[cfg(test)]` only; lopdf heavy imports gated to tests in `pdf_local.rs`"

requirements-completed: [PRE-14 subset, D-05/D-06 wiring for Messages API]

verification:
  - "cargo test (anthropic_assets_mock, pdf_raster unit tests)"

status: complete
completed_at: 2026-04-15
---

# 09-02 Summary — Anthropic vision + PDF raster

Delivered the HTTP vision client, environment overrides for mock servers, PDF→PNG rasterization with a CLI fallback, and wiremock coverage for the image message shape. Pinned the `image` crate to avoid a broken `png` dependency resolution on crates.io.
