---
phase: 5
slug: web-dashboard
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-10
---

# Phase 5 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) |
| **Config file** | none — Wave 0 installs stubs |
| **Quick run command** | `cargo test --lib 2>&1 | tail -20` |
| **Full suite command** | `cargo test 2>&1 | tail -30` |
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --lib 2>&1 | tail -20`
- **After every plan wave:** Run `cargo test 2>&1 | tail -30`
- **Before `/gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 5-01-01 | 01 | 0 | WEB-01 | — | N/A | compile | `cargo build 2>&1 | tail -5` | ❌ W0 | ⬜ pending |
| 5-01-02 | 01 | 1 | CLI-05 | — | N/A | integration | `cargo test serve 2>&1 | tail -10` | ❌ W0 | ⬜ pending |
| 5-01-03 | 01 | 1 | WEB-01 | — | N/A | integration | `cargo test router 2>&1 | tail -10` | ❌ W0 | ⬜ pending |
| 5-02-01 | 02 | 1 | WEB-02 | — | N/A | integration | `cargo test search_handler 2>&1 | tail -10` | ❌ W0 | ⬜ pending |
| 5-02-02 | 02 | 2 | WEB-03 | — | N/A | integration | `cargo test index_browser 2>&1 | tail -10` | ❌ W0 | ⬜ pending |
| 5-02-03 | 02 | 2 | WEB-04 | — | N/A | integration | `cargo test status_handler 2>&1 | tail -10` | ❌ W0 | ⬜ pending |
| 5-03-01 | 03 | 2 | WEB-05 | — | N/A | integration | `cargo test settings_handler 2>&1 | tail -10` | ❌ W0 | ⬜ pending |
| 5-03-02 | 03 | 3 | WEB-06 | — | N/A | compile | `cargo build --release 2>&1 | tail -5` | ✅ | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `tests/web_dashboard.rs` — stubs for WEB-01 through WEB-06
- [ ] `src/web/` module — mod.rs skeleton so compile tests pass

*Existing infrastructure covers cargo test; only test stubs need to be created.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Dashboard renders correctly in browser | WEB-01 | Visual rendering can't be automated in cargo test | Open `http://127.0.0.1:3000` and verify layout matches UI-SPEC |
| Search results display correctly | WEB-02 | Visual inspection required | Run a query, verify chunk text, file path, breadcrumb, and score appear |
| Index browser shows all files | WEB-03 | DB-dependent rendering | Verify file list shows chunk counts and timestamps |
| Settings view shows current config | WEB-05 | Config-dependent rendering | Verify all config values displayed read-only |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
