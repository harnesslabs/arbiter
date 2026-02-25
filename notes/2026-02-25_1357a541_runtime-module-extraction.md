---
date: 2026-02-25
commit: 1357a541
tags:
  - phase0
  - arbiter-core
  - runtime
  - refactor
related_components:
  - arbiter-core/src/agent.rs
  - arbiter-core/src/runtime/mod.rs
  - arbiter-core/src/runtime/in_memory.rs
  - arbiter-core/src/lib.rs
---

# Phase 0 Milestone: Runtime Module Extraction (In-Memory Loop)

## Summary

This cycle extracted the in-memory processing loop from `Agent::<_, InMemory>::process()` into a new `arbiter_core::runtime::in_memory` module and changed `Agent::process()` to delegate to that runtime module.

The behavior remains the same (tests stayed green), but the runtime logic now has a dedicated module boundary that future broker/node runtimes can build on.

## The Why

### Why do an organizational refactor before TCP/broker work

The in-memory processing loop had already grown to include:

- control-plane state transitions
- transport receive/send behavior
- recipient filtering
- stable kind/version dispatch fallback
- handler error logging

Keeping that all inside `agent.rs` would make LAN runtime work harder to factor later. Extracting now reduces future merge risk and creates an obvious home for runtime-specific logic.

### Why keep the extraction in `arbiter-core`

This aligns with the "crates only when needed" roadmap rule. The refactor establishes separation of concerns without paying the coordination and packaging cost of a new crate.

## Blockers & Solutions

### Blocker: `runtime` module needed access to `Agent` internals

- Problem: sibling modules cannot access private fields/methods in `Agent`.
- Solution: promoted a narrow set of internals to `pub(crate)` (`state`, `inner`, `connection`, handler registries, and `resolve_handler_type_id`) while keeping them non-public outside the crate.

### Blocker: avoiding behavior regressions during extraction

- Problem: the loop includes multiple recent changes (recipient filtering, wire fallback dispatch, structured handler errors), so it was easy to lose behavior while moving code.
- Solution: extracted with minimal logic changes and validated via the full existing `arbiter-core` test suite plus workspace `just lint` / `just test`.

## Fallback Plan

If the runtime module boundary causes friction:

1. Keep the `runtime` module files, but inline the `in_memory::spawn` implementation back into `Agent::<_, InMemory>::process()`.
2. Retain `pub(crate)` visibility changes temporarily to avoid churn.
3. Re-extract incrementally after TCP/broker runtime APIs are clearer.

This preserves the architectural direction while reducing immediate refactor pressure.

## Validation Performed

- `cargo test -p arbiter-core` (passed)
- `cargo fmt --all` (ran successfully; nightly-config warnings only)
- `just lint` (passed)
- `just test` (passed)

## Follow-On Work (Next Slice)

- Continue docs/examples alignment (stale API references and feature flags)
- Continue runtime modularization toward shared broker/node runtime utilities

