---
phase: 8
slug: search-ux-enhancements
status: draft
nyquist_compliant: true
wave_0_complete: true
created: 2026-04-14
---

# Phase 8 — Validation Strategy

> Rust project: use `cargo test` as primary automated verification.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | built-in `cargo test` + `tokio::test` where async |
| **Config file** | none |
| **Quick run command** | `cargo test` |
| **Full suite command** | `cargo test` |
| **Estimated runtime** | ~30–120 seconds (depends on LanceDB integration tests) |

---

## Sampling Rate

- **After every task commit:** `cargo test` (or scoped `cargo test <filter>` when faster)
- **After every plan wave:** `cargo test`
- **Before `/gsd-verify-work`:** Full suite green
- **Max feedback latency:** 120 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 08-01-01 | 01 | 1 | WEB-07 | T-08-01 | Rerank intent only from explicit param; disabled UI when no key | unit + grep | `cargo test web::` / `cargo test` | ✅ | ⬜ pending |
| 08-01-02 | 01 | 1 | WEB-08 | T-08-02 | Snippet output is escaped; mark tags only from server | unit | `cargo test highlight` | ✅ | ⬜ pending |
| 08-01-03 | 01 | 1 | WEB-07, WEB-08 | — | Templates compile; no double-escape | compile | `cargo test` + `cargo check` | ✅ | ⬜ pending |

---

## Wave 0 Requirements

Existing infrastructure covers all phase requirements — no Wave 0 install.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|---------------------|
| Disabled rerank tooltip in browser | WEB-07 | `title` attribute needs real UA | Hover checkbox when no `ANTHROPIC_API_KEY`; confirm tooltip text appears |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or documented manual steps
- [ ] Sampling continuity: tests run after logic tasks
- [ ] No watch-mode flags
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
