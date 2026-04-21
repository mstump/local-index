---
phase: 11
slug: vision-enrichment-idempotency
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-20
---

# Phase 11 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test |
| **Config file** | Cargo.toml (existing) |
| **Quick run command** | `cargo test -p local-index 2>&1 | tail -20` |
| **Full suite command** | `cargo test 2>&1 | tail -40` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p local-index 2>&1 | tail -20`
- **After every plan wave:** Run `cargo test 2>&1 | tail -40`
- **Before `/gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 11-01-01 | 01 | 1 | PRE-04 | — | Cache read short-circuits API call | unit | `cargo test cache` | ✅ | ⬜ pending |
| 11-01-02 | 01 | 1 | PRE-04 | — | Corrupt cache triggers re-fetch with WARN | unit | `cargo test corrupt_cache` | ✅ W0 | ⬜ pending |
| 11-01-03 | 01 | 1 | PRE-11 | — | Blockquote format matches D-04/D-05 | unit | `cargo test blockquote` | ✅ W0 | ⬜ pending |
| 11-01-04 | 01 | 1 | PRE-12 | — | Standalone image produces blockquote markdown | unit | `cargo test standalone_image` | ✅ W0 | ⬜ pending |
| 11-02-01 | 02 | 2 | PRE-10 | — | TextFirst PDF embedded images extracted per page | unit | `cargo test textfirst_embedded` | ✅ W0 | ⬜ pending |
| 11-02-02 | 02 | 2 | PRE-09 | — | NeedsVision blockquotes use correct format | unit | `cargo test needsvision_blockquote` | ✅ W0 | ⬜ pending |
| 11-02-03 | 02 | 2 | PRE-13 | — | README contains ephemeral-cache documentation | manual | `grep -c "asset-cache" README.md` | ✅ | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `src/pipeline/assets/tests/cache_idempotency.rs` — unit tests for cache read/write/corrupt paths (PRE-04)
- [ ] `src/pipeline/assets/tests/blockquote_format.rs` — unit tests for blockquote output format (PRE-11, PRE-12)
- [ ] `src/pipeline/assets/tests/textfirst_embedded.rs` — unit tests for per-page embedded image extraction (PRE-10)

*If existing test infrastructure covers these, use existing test files.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| End-to-end: re-run preprocessor on unchanged PDF produces same output | PRE-04 | Requires real Anthropic API key and sample PDF | Run `cargo run -- preprocess <pdf>` twice; verify second run logs "cache_hit = true" |
| README documents ephemeral-cache approach | PRE-13 | Doc change | Grep README for "asset-cache" and "ephemeral" |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
