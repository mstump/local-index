---
phase: 02-storage-embedding-pipeline
verified: 2026-04-10T00:00:00Z
status: passed
score: 9/9 must-haves verified
re_verification: false
---

# Phase 02: Storage & Embedding Pipeline Verification Report

**Phase Goal:** Operator can index a vault end-to-end with embeddings stored in LanceDB, with incremental re-indexing on unchanged content
**Verified:** 2026-04-10
**Status:** passed
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Chunker uses smart size-based splitting: CHUNK_SIZE_CHARS=3600, 15% overlap, scored break points, no splits inside code fences | VERIFIED | `chunker.rs` lines 9-13: constants present; `chunk_by_size`, `find_best_cutoff`, `find_code_fences` all implemented and tested |
| 2 | Chunk body includes heading text; heading_breadcrumb is active heading at chunk start as metadata | VERIFIED | `types.rs` line 31: doc comment "headings included in body for embedding quality"; `chunk_markdown` two-pass approach confirmed |
| 3 | VOYAGE_API_KEY env var resolved at startup; missing key produces clear error | VERIFIED | `credentials.rs`: `resolve_voyage_key()` returns `Err(Credential(...))` with URL guidance; test `test_resolve_voyage_key_unset` passes |
| 4 | Embedder trait exists with embed(), model_id(), dimensions() methods | VERIFIED | `embedder.rs` lines 7-19: trait defined with all three methods; `Send + Sync` bounds |
| 5 | VoyageEmbedder sends batched POST requests to Voyage AI API and returns Vec<Vec<f32>> | VERIFIED | `embedder.rs`: batch loop over `texts.chunks(batch_size)`, POST to `api.voyageai.com/v1/embeddings`, sorts by index; wiremock tests pass |
| 6 | Transient API errors (429, 5xx) trigger exponential backoff with jitter up to 5 retries | VERIFIED | `embedder.rs` lines 52-54: MAX_RETRIES=5, BASE_DELAY_MS=500; retry loop with `2u64.pow(attempt-1) + jitter`; `test_embed_retry_on_429` and `test_embed_retry_exhausted` pass |
| 7 | Auth errors (401/403) fail immediately without retry | VERIFIED | `embedder.rs` lines 139-145: immediate return on UNAUTHORIZED/FORBIDDEN; `test_embed_auth_failure` expects exactly 1 call |
| 8 | Chunks stored in LanceDB with all required metadata columns; incremental skip works | VERIFIED | `store.rs`: 10-column Arrow schema (chunk_id, file_path, heading_breadcrumb, body, line_start, line_end, frontmatter_json, content_hash, embedding_model, vector); `get_hashes_for_file` + hash-set comparison in `main.rs`; all store tests pass |
| 9 | Operator can run `local-index index <path>` end-to-end | VERIFIED | `main.rs`: fully wired pipeline — credentials → embedder → store → progress → summary; all integration tests pass |

**Score:** 9/9 truths verified

---

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `src/credentials.rs` | Credential resolution for VOYAGE_API_KEY | VERIFIED | 56 lines; exports `resolve_voyage_key()`; tests pass |
| `src/pipeline/embedder.rs` | Embedder trait and VoyageEmbedder implementation | VERIFIED | 412 lines; exports `Embedder`, `VoyageEmbedder`, `VoyageRequest`, `VoyageResponse`; 8 wiremock tests pass |
| `src/error.rs` | Error variants for Embedding, Database, Credential | VERIFIED | Contains `Credential(String)`, `Embedding(String)`, `Database(String)`, `is_transient()` |
| `src/pipeline/store.rs` | LanceDB store with all operations | VERIFIED | 553 lines (>150 min); all 9 public functions present; 12 tests pass |
| `src/main.rs` | Wired index command with embed+store+progress pipeline | VERIFIED | Contains all required symbols: `ChunkStore`, `VoyageEmbedder::new`, `resolve_voyage_key`, `check_model_consistency`, `compute_content_hash`, `ProgressBar`, `is_terminal()`, `eprintln!` |
| `tests/index_integration.rs` | End-to-end integration tests | VERIFIED | 255 lines (>50 min); 5 non-ignored tests pass; 1 `#[ignore]` for real API |

---

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `src/pipeline/embedder.rs` | `https://api.voyageai.com/v1/embeddings` | reqwest POST with Bearer token | VERIFIED | Line 96: `format!("{}/v1/embeddings", self.base_url)`; line 112: `Authorization: Bearer {api_key}` |
| `src/credentials.rs` | `std::env::var` | VOYAGE_API_KEY env var lookup | VERIFIED | Line 8: `std::env::var("VOYAGE_API_KEY")` |
| `src/pipeline/store.rs` | lancedb | `connect() -> Database -> Table` | VERIFIED | Line 70: `lancedb::connect(db_path).execute().await` |
| `src/pipeline/store.rs` | `src/types.rs` | Chunk struct fields mapped to Arrow RecordBatch columns | VERIFIED | Lines 119-162: all 10 Chunk fields mapped to Arrow arrays in RecordBatch |
| `src/main.rs` | `src/pipeline/store.rs` | ChunkStore::open, store_chunks, get_hashes_for_file, check_model_consistency | VERIFIED | Lines 66, 70, 178, 227, 219 respectively |
| `src/main.rs` | `src/pipeline/embedder.rs` | VoyageEmbedder::new, embed() | VERIFIED | Lines 65, 216 |
| `src/main.rs` | `src/credentials.rs` | resolve_voyage_key() | VERIFIED | Line 62 |
| `src/main.rs` | indicatif | ProgressBar for TTY, eprintln! for non-TTY | VERIFIED | Lines 101-112 (ProgressBar), lines 160-165, 203-209, 255-262 (eprintln!) |

---

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|---------------|--------|--------------------|--------|
| `src/main.rs` index command | `result.embeddings` | `embedder.embed(&texts).await` → VoyageEmbedder POST | Real API call (no hardcoded fallback) | FLOWING |
| `src/pipeline/store.rs` | `hashes` from `get_hashes_for_file` | `lancedb` query with `only_if(file_path = '...')` filter | Real DB query | FLOWING |
| `src/main.rs` summary | `files_indexed`, `chunks_embedded`, `chunks_skipped` | Counters incremented in processing loop | Derived from actual per-file processing | FLOWING |

---

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| Binary builds without error | `cargo build` | Exit 0 | PASS |
| Full test suite green | `cargo test` | 45 unit + 8 integration (1 ignored) = 53 tests, 0 failures | PASS |
| `test_index_no_credentials` | integration test | Exit non-zero, stderr contains "VOYAGE_API_KEY" and "https://dash.voyageai.com/" | PASS |
| `test_index_empty_vault` | integration test | Exit 0, stdout contains "files_indexed" | PASS |
| `test_index_json_output_non_tty` | integration test | Exit 0, valid JSON with all 4 keys | PASS |
| `test_index_force_reindex_flag` | integration test | Exit 0, flag accepted | PASS |
| `test_index_nonexistent_path` | integration test | Exit non-zero, stderr contains "Invalid vault path" | PASS |

---

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| CLI-01 | 02-03 | Operator can run `local-index index <path>` for one-shot full index | SATISFIED | `main.rs` full pipeline wired; integration tests confirm exit 0 on success |
| CRED-01 | 02-01 | VOYAGE_API_KEY env var checked for Voyage AI credentials | SATISFIED | `credentials.rs` line 8: `std::env::var("VOYAGE_API_KEY")` |
| CRED-02 | 02-01 | Embedder trait enables configurable embedding providers | SATISFIED | `embedder.rs` lines 7-19: `pub trait Embedder: Send + Sync` |
| CRED-03 | 02-01 | Startup fails with clear error if no valid credentials | SATISFIED | `credentials.rs`: actionable error with URL; integration test `test_index_no_credentials` passes |
| INDX-04 | 02-02 | SHA-256 content hash; unchanged chunks skipped on re-index | SATISFIED | `store.rs`: `compute_content_hash()` over body+breadcrumb+frontmatter; `main.rs` lines 185-210: hash-set comparison skips unchanged files |
| INDX-05 | 02-02 | Embeddings stored in LanceDB with all metadata columns | SATISFIED | `store.rs` schema: 10 columns including file_path, heading_breadcrumb, line_start, line_end, frontmatter_json, content_hash, embedding_model, vector |
| INDX-06 | 02-02 | Model mismatch warns and requires --force-reindex | SATISFIED | `store.rs` `check_model_consistency()`: Err with "Embedding model mismatch" and "--force-reindex" guidance; `main.rs` wires it at startup |
| INDX-07 | 02-01 | Exponential backoff with jitter on rate-limit/transient errors | SATISFIED | `embedder.rs`: MAX_RETRIES=5, BASE_DELAY_MS=500, jitter; wiremock retry tests pass |
| INDX-08 | 02-03 | Progress reporting during one-shot mode | SATISFIED | `main.rs`: TTY ProgressBar + non-TTY `eprintln!` per-file lines to stderr; JSON summary to stdout |

**All 9 requirements satisfied.** REQUIREMENTS.md correctly shows INDX-04/05/06 as pending (pre-phase status) — all three are now implemented and verified in code.

---

### Anti-Patterns Found

| File | Pattern | Severity | Impact |
|------|---------|----------|--------|
| `src/main.rs` lines 332-365 | Daemon, Search, Status, Serve commands are stubs (`tracing::warn!("not yet implemented")`) | INFO | These are future-phase commands, not part of Phase 2 goal. No impact on phase 2 goal. |

No blocker or warning-level anti-patterns found in Phase 2 code paths.

---

### Human Verification Required

#### 1. Real Voyage AI End-to-End Flow

**Test:** Set `VOYAGE_API_KEY` to a real key, run `cargo test -- test_index_with_real_api --ignored`
**Expected:** Exit 0, `chunks_embedded > 0`, `.local-index/` directory created with LanceDB files
**Why human:** Requires real Voyage AI API access; cannot mock in automated verification

#### 2. TTY Progress Bar Appearance

**Test:** Run `local-index index <vault>` in an interactive terminal with VOYAGE_API_KEY set and real markdown files
**Expected:** Animated indicatif progress bar visible with `[bar:40.cyan/blue]` style; bar clears on completion; summary line printed
**Why human:** TTY detection and terminal rendering cannot be verified programmatically

---

### Gaps Summary

No gaps found. All phase-2 must-haves are implemented, substantive, wired, and data-flowing. The full test suite passes: 45 unit tests + 5 active integration tests with 0 failures.

The two items under Human Verification are not gaps — they require a real API key or interactive terminal and cannot be automated without external infrastructure.

---

_Verified: 2026-04-10_
_Verifier: Claude (gsd-verifier)_
