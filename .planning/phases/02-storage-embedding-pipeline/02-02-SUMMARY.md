---
phase: 02-storage-embedding-pipeline
plan: 02
subsystem: storage
tags: [lancedb, arrow, vector-db, content-hash, sha256, incremental-indexing]
dependency_graph:
  requires: [02-01-embedding-pipeline-foundation]
  provides: [ChunkStore, compute_content_hash, chunks_schema, model-mismatch-guard]
  affects: [02-03-PLAN, 03-search]
tech_stack:
  added: []
  patterns: [delete-all-insert-new, arrow-recordbatch-construction, content-hash-skip-logic]
key_files:
  created: [src/pipeline/store.rs]
  modified: [src/pipeline/mod.rs]
key_decisions:
  - "Delete-all-for-file + insert-new pattern over merge_insert for simpler file-level updates"
  - "FixedSizeList(Float32, 1024) for embedding vector column matches Voyage AI dimensions"
  - "Content hash over body+breadcrumb+frontmatter enables incremental skip logic"
  - "Model mismatch returns Err unless --force-reindex, Ok(true) signals caller to clear"
patterns_established:
  - "LanceDB table access via QueryBase + ExecutableQuery traits"
  - "RecordBatchIterator wrapping for table.add() API"
  - "Arrow StringArray/UInt32Array/FixedSizeListArray column construction"
requirements-completed: [INDX-04, INDX-05, INDX-06]
duration: 24min
completed: 2026-04-10
---

# Phase 02 Plan 02: LanceDB Chunk Store Summary

**LanceDB ChunkStore with 10-column Arrow schema, SHA-256 content hashing for incremental re-indexing, and embedding model mismatch guard requiring --force-reindex**

## Performance

- **Duration:** 24 min
- **Started:** 2026-04-10T18:35:20Z
- **Completed:** 2026-04-10T18:59:45Z
- **Tasks:** 1
- **Files modified:** 2

## Accomplishments
- ChunkStore opens/creates LanceDB tables with correct 10-column Arrow schema
- Store, query, and delete chunks by file path for delete+insert update pattern
- SHA-256 content hash computed over body+breadcrumb+frontmatter for incremental skip logic
- Model mismatch guard blocks re-indexing with wrong model unless --force-reindex
- 13 tests covering all store operations and hash determinism

## Task Commits

Each task was committed atomically:

1. **Task 1: LanceDB ChunkStore with schema, upsert, hash query, and model guard** - `fc74e71` (feat)

## Files Created/Modified
- `src/pipeline/store.rs` - LanceDB ChunkStore: open/create table, store_chunks, get_hashes_for_file, delete_chunks_for_file, check_model_consistency, clear_all, compute_content_hash
- `src/pipeline/mod.rs` - Added `pub mod store;`

## Decisions Made
- Delete-all-for-file + insert-new pattern chosen over merge_insert for simplicity (entire files processed at a time)
- FixedSizeList(Float32, 1024) for embedding vector column matches Voyage AI voyage-3.5 dimensions
- Content hash computed over body+breadcrumb+serialized-frontmatter per D-09
- check_model_consistency returns Ok(false) for match/empty, Ok(true) for force-reindex, Err for mismatch

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] LanceDB API requires additional trait imports**
- **Found during:** Task 1 (compilation)
- **Issue:** LanceDB query methods (only_if, select, execute) require explicit trait imports (QueryBase, ExecutableQuery) not mentioned in plan
- **Fix:** Added `use lancedb::query::{ExecutableQuery, QueryBase};` and `use arrow_array::Array;`
- **Files modified:** src/pipeline/store.rs
- **Verification:** cargo check passes, all tests pass

**2. [Rule 3 - Blocking] SHA-256 format!("{:x}") incompatible with sha2 0.11 output type**
- **Found during:** Task 1 (compilation)
- **Issue:** sha2 0.11's GenericArray does not implement LowerHex trait, so `format!("{:x}", hasher.finalize())` fails
- **Fix:** Used iterator-based hex encoding: `result.iter().fold(String::new(), |mut acc, b| { write!(acc, "{:02x}", b).unwrap(); acc })`
- **Files modified:** src/pipeline/store.rs
- **Verification:** Hash tests pass, deterministic output confirmed

**3. [Rule 3 - Blocking] LanceDB table.add() requires RecordBatchIterator, not Vec<RecordBatch>**
- **Found during:** Task 1 (compilation)
- **Issue:** `table.add(vec![batch])` fails because Vec<RecordBatch> does not implement IntoArrow
- **Fix:** Wrapped in `RecordBatchIterator::new(vec![Ok(batch)], schema)`
- **Files modified:** src/pipeline/store.rs
- **Verification:** store_chunks test passes

---

**Total deviations:** 3 auto-fixed (3 blocking)
**Impact on plan:** All auto-fixes were necessary for LanceDB/Arrow API compatibility. No scope creep.

## Issues Encountered
- Worktree was missing Plan 02-01 changes; resolved with git merge from main

## Known Stubs

None -- all functionality is fully implemented and tested.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- ChunkStore ready for Plan 02-03 (pipeline orchestrator) to wire together walker+chunker+embedder+store
- All store operations tested with real LanceDB instances in temp directories

## Self-Check: PASSED

- All 2 key files verified present on disk (src/pipeline/store.rs, src/pipeline/mod.rs)
- Task commit verified in git log (fc74e71)
- `cargo test --lib` passes all 45 tests (0 failures)
- `cargo test store` passes all 13 store tests
- `cargo test content_hash` passes all 5 hash tests

---
*Phase: 02-storage-embedding-pipeline*
*Completed: 2026-04-10*
