---
phase: 2
slug: storage-embedding-pipeline
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-09
---

# Phase 2 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (built-in) + tokio::test for async |
| **Config file** | None needed (Cargo.toml [dev-dependencies]) |
| **Quick run command** | `cargo test` |
| **Full suite command** | `cargo test -- --include-ignored` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test`
- **After every plan wave:** Run `cargo test -- --include-ignored`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 2-01-01 | 01 | 0 | CRED-01, CRED-02, CRED-03 | unit | `cargo test credentials` | ❌ W0 | ⬜ pending |
| 2-01-02 | 01 | 0 | CRED-02 | unit | `cargo test embedder` | ❌ W0 | ⬜ pending |
| 2-01-03 | 01 | 0 | INDX-04, INDX-05, INDX-06 | unit | `cargo test store` | ❌ W0 | ⬜ pending |
| 2-01-04 | 01 | 0 | INDX-07 | unit (wiremock) | `cargo test retry` | ❌ W0 | ⬜ pending |
| 2-01-05 | 01 | 0 | CLI-01, INDX-08 | integration | `cargo test --test index_integration` | ✅ (needs update) | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `src/credentials.rs` — unit tests for VOYAGE_API_KEY resolution (CRED-01, CRED-03)
- [ ] `src/pipeline/embedder.rs` — unit tests for `Embedder` trait + `VoyageEmbedder` with wiremock (CRED-02)
- [ ] `src/pipeline/store.rs` — unit tests for LanceDB schema, content hash, model mismatch guard (INDX-04, INDX-05, INDX-06)
- [ ] `src/pipeline/embedder.rs` retry tests — exponential backoff with wiremock (INDX-07)
- [ ] Update `tests/index_integration.rs` — end-to-end index with mocked embedder (CLI-01, INDX-08)
- [ ] Add `wiremock = "^0.6"` to `[dev-dependencies]` in Cargo.toml

*`tempfile` already present in dev-dependencies from Phase 1 — reuse for LanceDB temp dirs.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| TTY progress bar renders correctly | INDX-08 | Cannot assert visual terminal output in cargo test | Run `cargo run -- index <vault-path>` in a real terminal; verify indicatif bar renders and summary line appears on completion |
| Non-TTY JSON summary to stdout | INDX-08 | Pipe detection differs in CI vs local | Run `cargo run -- index <vault-path> \| cat` and verify stdout contains one JSON line with `files_indexed`, `chunks_embedded`, `chunks_skipped`, `errors` fields |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
