---
phase: 02-storage-embedding-pipeline
plan: 03
subsystem: pipeline
tags: [indexing, embeddings, lancedb, indicatif, progress, incremental]

# Dependency graph
requires:
  - phase: 02-01
    provides: "VoyageEmbedder, Embedder trait, credentials resolution"
  - phase: 02-02
    provides: "ChunkStore with store_chunks, get_hashes_for_file, check_model_consistency"
provides:
  - "End-to-end index command: walk -> chunk -> hash -> skip/embed -> store -> progress"
  - "TTY-aware progress reporting (indicatif bar vs stderr lines)"
  - "JSON summary output on stdout for non-TTY consumers"
  - "Incremental indexing via content hash comparison"
affects: [search, daemon, claude-integration]

# Tech tracking
tech-stack:
  added: [indicatif]
  patterns: [TTY-detection for output mode, content-hash-based skip logic, stderr progress + stdout JSON]

key-files:
  created: []
  modified:
    - src/main.rs
    - tests/index_integration.rs

key-decisions:
  - "Redirect tracing logs to stderr to keep stdout clean for JSON summary"
  - "Whole-file re-embedding on any chunk change (simpler than per-chunk diffing with overlapping chunks)"
  - "Set child process cwd in integration tests to prevent dotenvy loading project .env"

patterns-established:
  - "TTY detection: is_terminal() gates indicatif vs eprintln! progress"
  - "Non-TTY output contract: JSON summary to stdout, progress lines to stderr"
  - "Integration test isolation: set current_dir to temp dir, explicit env var control"

requirements-completed: [CLI-01, INDX-08]

# Metrics
duration: 40min
completed: 2026-04-10
---

# Phase 02 Plan 03: Index Pipeline Wiring Summary

**Wired index command to full embed+store pipeline with async main, incremental skip logic, TTY-aware indicatif progress bars, and JSON summary output for non-TTY consumers**

## Performance

- **Duration:** 40 min
- **Started:** 2026-04-10T19:07:52Z
- **Completed:** 2026-04-10T19:48:02Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments
- Index command now runs the full pipeline: resolve credentials, create embedder+store, check model consistency, walk/chunk files, compute content hashes, skip unchanged files, embed changed chunks, store in LanceDB
- TTY mode shows indicatif progress bar; non-TTY mode emits per-file progress to stderr and JSON summary to stdout
- Integration tests cover: missing credentials error, empty vault, --force-reindex flag, nonexistent path, JSON output format validation

## Task Commits

Each task was committed atomically:

1. **Task 1: Wire index command to embed+store pipeline with progress reporting** - `3857039` (feat)
2. **Task 2: Update integration tests for embed+store pipeline** - `cec0bf7` (test)

## Files Created/Modified
- `src/main.rs` - Async main with full embed+store pipeline, TTY-aware progress, JSON summary
- `tests/index_integration.rs` - Integration tests for credential errors, empty vault, flag acceptance, JSON output

## Decisions Made
- Redirected tracing logs to stderr (`with_writer(std::io::stderr)`) to prevent log output from corrupting stdout JSON summary
- Used whole-file re-embedding strategy: if any chunk hash changes, delete all old chunks and re-embed entire file. Simpler and correct for overlapping smart chunks.
- Set child process `current_dir` in integration tests to temp directories to prevent dotenvy from loading the project's `.env` file

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Tracing output redirected to stderr**
- **Found during:** Task 1 (wiring index command)
- **Issue:** Default tracing subscriber writes to stdout, which would corrupt the JSON summary output in non-TTY mode
- **Fix:** Added `.with_writer(std::io::stderr)` to the tracing subscriber initialization
- **Files modified:** src/main.rs
- **Verification:** Running binary piped shows clean JSON on stdout, tracing on stderr
- **Committed in:** 3857039

**2. [Rule 3 - Blocking] Integration test dotenvy isolation**
- **Found during:** Task 2 (integration tests)
- **Issue:** `test_index_no_credentials` was passing unexpectedly because dotenvy loaded VOYAGE_API_KEY from the project's `.env` file even after `env_remove`
- **Fix:** Set child process `current_dir` to the vault temp directory so dotenvy cannot find `.env`
- **Files modified:** tests/index_integration.rs
- **Verification:** `test_index_no_credentials` now correctly fails when VOYAGE_API_KEY is not set
- **Committed in:** cec0bf7

---

**Total deviations:** 2 auto-fixed (1 bug, 1 blocking)
**Impact on plan:** Both fixes essential for correctness. Tracing to stderr is required for the non-TTY JSON output contract. Test isolation is required for reliable CI.

## Issues Encountered
None beyond the auto-fixed deviations above.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Phase 2 complete: full index pipeline from vault walk through embedding storage
- Ready for Phase 3 (Search): semantic search, full-text search, hybrid search over stored chunks
- Ready for Phase 4 (Daemon): file watcher can trigger re-indexing using the same pipeline

## Self-Check: PASSED

- All created/modified files verified present
- Commit 3857039 (Task 1) verified in git log
- Commit cec0bf7 (Task 2) verified in git log

---
*Phase: 02-storage-embedding-pipeline*
*Completed: 2026-04-10*
