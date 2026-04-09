---
phase: 1
slug: foundation-file-processing
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-08
---

# Phase 1 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `cargo test` (Rust built-in) |
| **Config file** | `Cargo.toml` — test modules inline and in `tests/` |
| **Quick run command** | `cargo test --lib` |
| **Full suite command** | `cargo test` |
| **Estimated runtime** | ~5 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --lib`
- **After every plan wave:** Run `cargo test`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 10 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| cargo project init | 01 | 0 | CLI-06 | compile | `cargo build` | ❌ W0 | ⬜ pending |
| CLI skeleton (clap) | 01 | 1 | CLI-06, CLI-07 | unit + compile | `cargo test --lib` | ❌ W0 | ⬜ pending |
| tracing setup | 01 | 1 | CLI-08 | unit | `cargo test --lib` | ❌ W0 | ⬜ pending |
| walkdir traversal | 02 | 1 | INDX-01 | unit | `cargo test --lib` | ❌ W0 | ⬜ pending |
| markdown chunker | 02 | 1 | INDX-02 | unit | `cargo test --lib` | ❌ W0 | ⬜ pending |
| frontmatter parser | 02 | 1 | INDX-03 | unit | `cargo test --lib` | ❌ W0 | ⬜ pending |
| chunk data structure | 03 | 0 | INDX-02, INDX-03 | compile + unit | `cargo test --lib` | ❌ W0 | ⬜ pending |
| integration test | 03 | 2 | all Phase 1 | integration | `cargo test` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `src/main.rs` — binary entry point (minimal)
- [ ] `src/lib.rs` — library root with module declarations
- [ ] `src/cli.rs` — clap CLI structs (stubs)
- [ ] `src/chunk.rs` — Chunk struct definition (must compile, no logic yet)
- [ ] `tests/chunker_tests.rs` — test stubs for chunker edge cases
- [ ] `Cargo.toml` — with all Phase 1 dependencies declared

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| `--help` output is human-readable with examples | CLI-06 | Subjective readability | Run `cargo run -- --help` and each subcommand `--help`; verify examples present |
| RUST_LOG controls verbosity | CLI-08 | Environment interaction | Run `RUST_LOG=trace cargo run -- index /tmp/test-vault` and verify trace output |
| .env file loaded before CLI parse | CLI-07 | Environment interaction | Create `.env` with `LOCAL_INDEX_LOG_LEVEL=debug`, run without flag, verify debug logs |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 10s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
