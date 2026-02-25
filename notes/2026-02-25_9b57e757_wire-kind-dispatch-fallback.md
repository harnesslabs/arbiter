---
date: 2026-02-25
commit: 9b57e757
tags:
  - phase0
  - arbiter-core
  - dispatch
  - message-kind
  - schema-version
related_components:
  - arbiter-core/src/agent.rs
  - notes/roadmap.md
  - notes/active/2026-02-25_phase0-core-contract-cleanup.md
---

# Phase 0 Milestone: Stable Kind/Version Dispatch Fallback

## Summary

This cycle completed the Phase 0 roadmap milestone for stable identifier-based handler registration and dispatch fallback:

- `Agent` now stores a wire-facing handler registry keyed by `(MessageKind, SchemaVersion)`
- `with_handler::<M>()` auto-registers a default wire mapping using `MessageKind::for_type::<M>()` and schema version `1`
- added `with_handler_kind::<M>(kind, version)` for explicit wire registration
- in-memory dispatch now falls back from `TypeId` lookup to the wire registry
- tests prove dispatch still works when the incoming envelope `TypeId` is intentionally wrong/unusable

## The Why

### Why this milestone follows envelope metadata immediately

The previous milestone added wire metadata to envelopes but still selected handlers using `TypeId`. That left a critical gap: the runtime could carry stable wire identity while still being unable to dispatch solely from it.

This milestone closes that gap in `arbiter-core` without introducing new crates or prematurely building TCP/broker infrastructure.

### Why keep `TypeId` as the first lookup and use wire dispatch as fallback

- local in-memory flows still benefit from direct `TypeId` lookup
- existing behavior remains fast and unchanged for common local tests
- the fallback path makes wire-driven dispatch testable now
- it reduces migration risk while preserving a clear path to future transport-driven runtimes

### Why add `with_handler_kind` instead of forcing explicit kinds everywhere

The codebase is still in heavy pre-1.0 iteration. Requiring explicit wire kinds for every existing handler today would create unnecessary migration churn. `with_handler_kind` gives an explicit path for stable remote contracts while `with_handler` remains the ergonomic default.

## Blockers & Solutions

### Blocker: proving wire dispatch without a real network transport

- Problem: local envelopes still naturally carry valid `TypeId`, so fallback dispatch would not be exercised in normal tests.
- Solution: tests intentionally mutate `envelope.type_id` to a non-matching type and verify dispatch succeeds via wire metadata registry.

### Blocker: avoiding handler registry duplication bugs

- Problem: multiple registration paths (`with_handler`, `with_handler_kind`) could create duplicate handler closures for the same message type.
- Solution: handler closures are inserted with `entry(...).or_insert_with(...)`, while wire mappings can alias to the same handler `TypeId`.

## Fallback Plan

If the fallback dispatch path causes regressions:

1. Keep `with_handler_kind` and the wire registry data structure, but disable runtime fallback lookup (use `TypeId` only).
2. Continue generating envelope metadata and recording stable kinds in tests/docs.
3. Reintroduce wire dispatch behind a feature flag or explicit runtime mode after TCP/broker code exists.

This would preserve the API surface while narrowing runtime behavior back to the previous known-good path.

## Validation Performed

- `cargo test -p arbiter-core` (passed)
- `cargo fmt --all` (ran successfully; nightly-config warnings only)
- `just lint` (passed)
- `just test` (passed)

## Follow-On Work (Next Slice)

- Extract shared runtime flow from `Agent::<_, InMemory>::process` into `arbiter_core::runtime` modules.
- Start API/docs/example alignment pass (remove stale `runtime::Runtime` references in examples).
- Preserve the new wire registry semantics while adding transport-facing runtime abstractions.

