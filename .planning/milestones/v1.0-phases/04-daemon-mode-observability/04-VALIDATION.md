---
phase: 4
slug: daemon-mode-observability
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-10
---

# Phase 4 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (built-in Rust test framework) |
| **Config file** | Cargo.toml |
| **Quick run command** | `cargo test 2>&1` |
| **Full suite command** | `cargo test -- --include-ignored 2>&1` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test 2>&1`
- **After every plan wave:** Run `cargo test -- --include-ignored 2>&1`
- **Before `/gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 4-01-01 | 01 | 1 | WTCH-01 | — | N/A | unit | `cargo test watcher` | ✅ | ⬜ pending |
| 4-01-02 | 01 | 1 | WTCH-02 | — | N/A | unit | `cargo test file_event` | ✅ | ⬜ pending |
| 4-01-03 | 01 | 1 | WTCH-03 | — | N/A | unit | `cargo test rename` | ✅ | ⬜ pending |
| 4-01-04 | 01 | 1 | WTCH-04 | — | N/A | unit | `cargo test graceful_shutdown` | ✅ | ⬜ pending |
| 4-02-01 | 02 | 1 | CLI-02 | — | N/A | unit | `cargo test daemon_command` | ✅ | ⬜ pending |
| 4-02-02 | 02 | 1 | CLI-04 | — | N/A | unit | `cargo test status_command` | ✅ | ⬜ pending |
| 4-03-01 | 03 | 2 | OBS-01 | — | N/A | unit | `cargo test metrics` | ✅ | ⬜ pending |
| 4-03-02 | 03 | 2 | OBS-02 | — | N/A | integration | `cargo test metrics_endpoint` | ✅ | ⬜ pending |
| 4-03-03 | 03 | 2 | OBS-03 | — | N/A | unit | `cargo test hdr_histogram` | ✅ | ⬜ pending |
| 4-03-04 | 03 | 2 | OBS-04 | — | N/A | unit | `cargo test health_endpoint` | ✅ | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `src/daemon/mod.rs` — stubs for WTCH-01, WTCH-02, WTCH-03, WTCH-04
- [ ] `src/cli/daemon.rs` — stubs for CLI-02
- [ ] `src/cli/status.rs` — stubs for CLI-04
- [ ] `src/metrics/mod.rs` — stubs for OBS-01, OBS-02, OBS-03, OBS-04

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Daemon watches live vault directory | WTCH-01 | Requires running process + file system interaction | Run `cargo run -- daemon /tmp/test-vault`, create/modify/delete a .md file, observe logs |
| Graceful shutdown timing | WTCH-04 | Requires SIGINT/SIGTERM signal delivery | Start daemon, run `kill -SIGTERM <pid>`, verify clean exit within 5s |
| Prometheus scrape format | OBS-02 | Requires live HTTP server | Start daemon, `curl localhost:PORT/metrics`, verify Prometheus exposition format |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
