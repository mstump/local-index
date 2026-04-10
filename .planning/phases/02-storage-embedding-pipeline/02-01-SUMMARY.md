---
phase: 02-storage-embedding-pipeline
plan: 01
subsystem: embedding-pipeline
tags: [chunker, embedder, credentials, voyage-ai, retry]
dependency_graph:
  requires: [01-foundation-file-processing]
  provides: [Embedder-trait, VoyageEmbedder, smart-chunking, credential-resolution]
  affects: [02-02-PLAN, 02-03-PLAN]
tech_stack:
  added: [lancedb, arrow-array, arrow-schema, reqwest, sha2, indicatif, futures, uuid, rand, wiremock]
  patterns: [trait-based-abstraction, exponential-backoff-with-jitter, smart-chunking]
key_files:
  created: [src/pipeline/embedder.rs, src/credentials.rs]
  modified: [src/pipeline/chunker.rs, src/types.rs, src/error.rs, src/lib.rs, src/pipeline/mod.rs, Cargo.toml, tests/index_integration.rs]
decisions:
  - "Smart chunking replaces one-chunk-per-heading: CHUNK_SIZE_CHARS=3600, 15% overlap, scored break points"
  - "Embedder trait with async embed() enables future provider additions without pipeline changes"
  - "VoyageEmbedder targets voyage-3.5 model, 1024 dimensions, 50 texts per batch"
  - "VOYAGE_API_KEY env var only, no ~/.claude/ fallback (per D-04)"
  - "BTreeMap for frontmatter extra field ensures deterministic serialization for content hashing"
metrics:
  duration: 24min
  completed: 2026-04-10T17:46:05Z
  tasks: 3
  files: 9
---

# Phase 02 Plan 01: Embedding Pipeline Foundation Summary

Smart chunking with scored break points, Embedder trait abstraction, and VoyageEmbedder with retry logic for Voyage AI API.

## Completed Tasks

| Task | Name | Commit | Key Files |
|------|------|--------|-----------|
| 1 | Rewrite chunker with smart size-based chunking | 1770faf | src/pipeline/chunker.rs, src/types.rs |
| 2 | Add dependencies, error variants, credentials, new types | 6fbe764 | Cargo.toml, src/error.rs, src/types.rs, src/credentials.rs, src/lib.rs |
| 3 | Embedder trait and VoyageEmbedder with retry logic | dcb5d9f | src/pipeline/embedder.rs, src/pipeline/mod.rs |

## What Was Built

### Smart Chunking (Task 1)

Replaced the Phase 1 one-chunk-per-heading approach with qmd-style smart size-based chunking:

- **Constants**: CHUNK_SIZE_CHARS=3600, CHUNK_OVERLAP_CHARS=540 (15%), CHUNK_WINDOW_CHARS=800
- **Break point scoring**: h1=100, h2=90, h3/code=80, h4=70, h5/hr=60, h6=50, blank=20, list=5, newline=1
- **Distance decay**: Squared distance decay with 0.7 factor prefers breaks closer to target
- **Code fence safety**: No chunk boundary inside code fences
- **Body semantics change**: Chunk body is now raw markdown (headings included) for better embedding quality
- **heading_breadcrumb**: Tracks active heading at chunk start position (metadata only)

### Credential Resolution (Task 2)

- `resolve_voyage_key()` checks VOYAGE_API_KEY env var only (per D-04)
- Clear actionable error message with URL to get API key (per D-05)
- BTreeMap replaces HashMap in Frontmatter.extra for deterministic serialization (per D-09)

### Embedder Trait and VoyageEmbedder (Task 3)

- **Embedder trait**: `embed()`, `model_id()`, `dimensions()` -- designed for easy provider addition
- **VoyageEmbedder**: POST to api.voyageai.com/v1/embeddings, voyage-3.5, 1024 dims
- **Batching**: 50 texts per batch, concatenated results
- **Retry**: Exponential backoff with jitter, max 5 retries for 429/5xx
- **Auth errors**: 401/403 fail immediately without retry

### New Dependencies

lancedb 0.26.2, arrow-array/schema 57, reqwest 0.12, sha2 0.11, indicatif 0.18, futures 0.3, uuid 1.0, rand 0.8, wiremock 0.6

### New Error Variants

Credential(String), Embedding(String), Database(String) with is_transient() helper

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Integration test update for new chunking semantics**
- **Found during:** Task 1
- **Issue:** Phase 1 integration test `test_index_heading_breadcrumbs` expected one-chunk-per-heading behavior
- **Fix:** Updated test to verify single-chunk output with heading-inclusive body and correct breadcrumb
- **Files modified:** tests/index_integration.rs
- **Commit:** 1770faf

**2. [Rule 3 - Blocking] Unsafe env var manipulation in Rust 2024 edition**
- **Found during:** Task 2
- **Issue:** `std::env::set_var` and `remove_var` are unsafe in Rust 2024 edition
- **Fix:** Wrapped in `unsafe` blocks in test code with safety comments
- **Files modified:** src/credentials.rs
- **Commit:** 6fbe764

## Decisions Made

1. **Smart chunking constants**: CHUNK_SIZE_CHARS=3600 (~900 tokens), 15% overlap, 800-char look-back window
2. **Embedder as impl trait**: Used `impl Future` in trait method instead of `async_trait` macro to avoid extra dependency
3. **Batching at 50**: Default batch size of 50 texts per Voyage AI API request
4. **Retry parameters**: BASE_DELAY_MS=500, MAX_DELAY_MS=30000, MAX_RETRIES=5

## Verification

- `cargo build` exits 0
- `cargo test` exits 0 (all 40 tests pass: 22 lib, 3 CLI integration, 5 index integration, 8 embedder, 2 credential)
- `cargo test chunker` passes all smart-chunking tests
- `cargo test embedder` passes all Embedder/VoyageEmbedder tests
- `cargo test credentials` passes credential resolution tests

## Known Stubs

None -- all functionality is fully implemented and tested.

## Self-Check: PASSED

- All 9 key files verified present on disk
- All 3 task commits verified in git log (1770faf, 6fbe764, dcb5d9f)
- `cargo test` passes all tests (0 failures)
- `cargo build` compiles successfully
