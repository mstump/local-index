---
phase: 3
slug: search
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-10
---

# Phase 3 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (built-in) + tokio::test |
| **Config file** | Cargo.toml [dev-dependencies] |
| **Quick run command** | `cargo test --lib` |
| **Full suite command** | `cargo test` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --lib`
- **After every plan wave:** Run `cargo test`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 3-01-01 | 01 | 0 | SRCH-01..10, CLI-03 | unit/integration | `cargo test` | ❌ W0 | ⬜ pending |
| 3-02-01 | 02 | 1 | SRCH-01, SRCH-02, SRCH-03 | integration | `cargo test --test search_integration` | ❌ W0 | ⬜ pending |
| 3-02-02 | 02 | 1 | SRCH-04, SRCH-09 | unit | `cargo test search::types::tests search::formatter::tests` | ❌ W0 | ⬜ pending |
| 3-02-03 | 02 | 1 | SRCH-05, SRCH-06 | unit | `cargo test search::tests` | ❌ W0 | ⬜ pending |
| 3-02-04 | 02 | 1 | SRCH-07, SRCH-08 | integration | `cargo test --test search_integration` | ❌ W0 | ⬜ pending |
| 3-02-05 | 02 | 1 | SRCH-10 | integration | `cargo test --test search_integration` | ❌ W0 | ⬜ pending |
| 3-02-06 | 02 | 2 | CLI-03 | integration | `cargo test --test search_integration` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `tests/search_integration.rs` — integration tests for all search modes against real LanceDB (tempdir): test_semantic_search, test_fts_search, test_hybrid_search, test_path_filter, test_tag_filter, test_context_chunks
- [ ] `src/search/types.rs` — unit tests for SearchResult serialization, score normalization
- [ ] `src/search/formatter.rs` — unit tests for pretty and JSON formatting

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Pretty output renders readably on narrow terminals | SRCH-09 | Terminal width varies, not easily automated | Run `local-index search "query" --format pretty` on 80-col terminal; verify no misalignment |
| Score ordering (higher = better) is intuitive | SRCH-01..03 | Subjective ranking quality | Index real vault, compare top results against expected relevance |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
