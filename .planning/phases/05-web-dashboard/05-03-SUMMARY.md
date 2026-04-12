---
phase: 05-web-dashboard
plan: 03
subsystem: web-dashboard
tags: [index-browser, status-page, settings-page, askama, chunk-store]
dependency_graph:
  requires: [web-module, dashboard-router, base-template, app-state]
  provides: [index-page, status-page, settings-page, count-chunks-per-file]
  affects: [web-handlers, pipeline-store]
tech_stack:
  added: []
  patterns: [hashmap-aggregation, kv-table-layout, conditional-section-rendering]
key_files:
  created: []
  modified:
    - src/pipeline/store.rs
    - src/web/handlers.rs
    - templates/index.html
    - templates/status.html
    - templates/settings.html
    - templates/base.html
decisions:
  - "Embedding Stats section rendered unconditionally on status page so model info is visible even with empty index"
  - "Token usage shows N/A because Voyage API does not return token counts in v1"
  - "Last indexed column shows em-dash since v1 schema has no timestamp field"
metrics:
  duration_seconds: 886
  completed: "2026-04-12T23:51:48Z"
  tasks_completed: 2
  tasks_total: 2
  files_created: 0
  files_modified: 6
---

# Phase 05 Plan 03: Index Browser, Status, and Settings Pages Summary

Index browser with per-file chunk counts from ChunkStore, status page with real totals and always-visible embedding stats (model voyage-3.5, dimensions 1024, token usage N/A), settings page with config display showing credential source not key value.

## Completed Tasks

| Task | Name | Commit | Key Files |
|------|------|--------|-----------|
| 1 | Add count_chunks_per_file and wire handlers | 5bd8163 | src/pipeline/store.rs, src/web/handlers.rs |
| 2 | Implement index, status, and settings templates | df08bb2 | templates/index.html, templates/status.html, templates/settings.html, templates/base.html |

## Decisions Made

1. **Embedding Stats always visible**: The Embedding Stats section on the status page renders unconditionally (outside the `total_chunks == 0` guard) so that model ID and dimensions are visible even before any indexing occurs.

2. **Token usage N/A**: The Voyage API does not return token counts in v1, so the Token Usage field displays "N/A" as specified by WEB-05.

3. **Last indexed em-dash**: The v1 schema has no last-indexed timestamp column, so the Last Indexed field shows an em-dash for each file per WEB-03.

## Deviations from Plan

None - plan executed exactly as written.

## Verification Results

- `cargo check` passes (all templates compile at build time)
- `cargo test --lib` passes: 72 tests, 0 failures
- count_chunks_per_file returns sorted file list with counts via HashMap aggregation
- Status page shows em-dash for last_index_time
- Status page shows "N/A" for token_usage
- Embedding Stats section renders even when total_chunks == 0
- Index table has "Last Indexed" column showing em-dash for each file
- Settings page shows credential_source from config, never raw API key value
- No `|safe` filter used in any template (askama auto-escaping active)

## Self-Check: PASSED

All 6 modified files exist. Both task commits (5bd8163, df08bb2) verified in git log.
