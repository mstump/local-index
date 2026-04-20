---
phase: 10
slug: ocr-providers
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-20
---

# Phase 10 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in tests + `cargo test` |
| **Config file** | `Cargo.toml` (dev-deps: `wiremock`) |
| **Quick run command** | `cargo test -p local-index -- <filter>` |
| **Full suite command** | `cargo test` |
| **Estimated runtime** | ~60–120 seconds (project size dependent) |

---

## Sampling Rate

- **After every task commit:** Run scoped `cargo test` for the module touched (see plan `<verify>`).
- **After every plan wave:** Run `cargo test`.
- **Before `/gsd-verify-work`:** Full `cargo test` must be green.
- **Max feedback latency:** ~120 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 10-01-T1 | 01 | 1 | PRE-07 | T-10-01 | No secrets in logs | unit | `cargo test` (ingest / OCR dispatch) | ⬜ | ⬜ pending |
| 10-01-T2 | 01 | 1 | PRE-07 | T-10-01 | TLS via reqwest | integration | `cargo test --test anthropic_assets_mock` | ✅ | ⬜ pending |
| 10-02-T1 | 02 | 1 | PRE-08 | T-10-02 | Bearer not logged | unit | `cargo test` credentials | ⬜ | ⬜ pending |
| 10-02-T2 | 02 | 1 | PRE-07/08 | T-10-02 | HTTPS to googleapis | integration | `cargo test --test document_ai_mock` (new) | ⬜ | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- Existing infrastructure covers Rust tests; **no separate Wave 0 install** unless planner adds a new integration test binary.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Real Google Document AI processor | PRE-08 | Costs + cloud project | Set env + run `local-index index` on a scanned PDF; confirm chunks contain OCR text |

*Automated tests use wiremock only.*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or documented manual exception
- [ ] Sampling continuity: tests after each wave
- [ ] Feedback latency acceptable for CI
- [ ] `nyquist_compliant: true` set in frontmatter when execution completes

**Approval:** pending
