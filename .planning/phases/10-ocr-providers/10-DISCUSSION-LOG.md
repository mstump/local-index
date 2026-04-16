# Phase 10: OCR providers - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.  
> Decisions are captured in `10-CONTEXT.md` — this log records choices for `/gsd-next` auto-advancement when no interactive session ran.

**Date:** 2026-04-16  
**Phase:** 10-OCR providers  
**Areas discussed:** Provider model, Configuration, Output contract (consolidated pass)

---

## Provider model

| Option | Description | Selected |
|--------|-------------|----------|
| Anthropic-only refactor | Keep single provider, only tidy code |  |
| Trait + Anthropic + Google | Pluggable OCR, default Anthropic, optional Google | ✓ |
| Separate binary for Document AI | New process for Google |  |

**User's choice:** Trait (or enum) with Anthropic default and optional Google Document AI — aligns with PRE-08 and existing `ingest.rs` integration.  
**Notes:** Phase 9 already implements rasterization + Anthropic per page; Phase 10 generalizes that path.

---

## Configuration

| Option | Description | Selected |
|--------|-------------|----------|
| Env only | Minimal surface |  |
| Env + CLI on index/daemon | Consistent with Phase 9 D-07 / project patterns | ✓ |

**User's choice:** Env + matching CLI flags on `index` and `daemon`.  
**Notes:** Exact names left to planner.

---

## Output contract

| Option | Description | Selected |
|--------|-------------|----------|
| Provider-native JSON stored | New index fields |  |
| Markdown-only for chunker | Same pipeline as today | ✓ |

**User's choice:** Map all provider output to markdown consumed by existing chunking.

---

## Claude's Discretion

- Document AI SDK shape, batching, retry, exact credential env names.

## Deferred Ideas

- Phase 11 items (vision polish, blockquotes, reassembly, idempotency) explicitly deferred per ROADMAP.
