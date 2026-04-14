---
phase: 09
slug: preprocessor-foundation
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-14
---

# Phase 9 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in tests (`cargo test`) |
| **Config file** | `Cargo.toml` |
| **Quick run command** | `cargo test` |
| **Full suite command** | `cargo test` |
| **Estimated runtime** | ~60–120 seconds (depends on integration tests and fixtures) |

---

## Sampling Rate

- **After every task commit:** Run `cargo test`
- **After every plan wave:** Run `cargo test`
- **Before `/gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** 120 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 09-01-01 | 01 | 1 | PRE-05, PRE-06 | T-09-01 | Caps on read size; paths confined to vault | unit + integration | `cargo test` | ⬜ W0 | ⬜ pending |
| 09-01-02 | 01 | 1 | PRE-03 | T-09-02 | No execution of ignored paths | unit | `cargo test` | ⬜ W0 | ⬜ pending |
| 09-02-01 | 02 | 2 | PRE-14, D-05/D-06 | T-09-03 | API key from env only; TLS via rustls | unit + wiremock | `cargo test` | ⬜ W0 | ⬜ pending |
| 09-03-01 | 03 | 3 | PRE-01, PRE-02, PRE-13 | T-09-04 | Daemon only processes paths under vault root | integration | `cargo test` | ⬜ W0 | ⬜ pending |
| 09-03-02 | 03 | 3 | PRE-01, PRE-13 | — | N/A docs | manual grep | `rg "asset" README.md` | ⬜ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `tests/` or `src/pipeline/assets/` — unit tests for classification / ignore filtering (stubs acceptable before implementation)
- [ ] Existing `wiremock` dev-dependency — reuse for Anthropic HTTP mocks

*Wave 0 is satisfied once Plan 09-01 adds the first `#[cfg(test)]` module for asset helpers.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| End-to-end index of real Obsidian vault PDF | PRE-06 | Fixture diversity | Run `local-index index /path/to/vault` with a known text PDF; `local-index search` for unique string |
| Anthropic vision with live key | D-05/D-06 | Costs money | Set keys; index one scanned PDF; confirm non-empty chunks |

*If none: "All phase behaviors have automated verification."* → **False** — live API check remains manual.

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 120s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
