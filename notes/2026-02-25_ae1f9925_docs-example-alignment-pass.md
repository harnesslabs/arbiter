---
date: 2026-02-25
commit: ae1f9925
tags:
  - phase0
  - docs
  - examples
  - api-alignment
related_components:
  - examples/leader/Cargo.toml
  - examples/leader/Cargo.lock
  - examples/leader/src/lib.rs
  - docs/src/index.md
  - notes/active/2026-02-25_phase0-core-contract-cleanup.md
---

# Phase 0 Milestone: Docs/Example API Alignment Pass (Initial)

## Summary

This cycle started the Phase 0 docs/example alignment cleanup and fixed concrete drift in the `examples/leader` example:

- removed stale `arbiter_core::runtime::Runtime` import reference
- updated `Leader` and `Follower` `LifeCycle` impls to match the current trait shape
- removed nonexistent `arbiter-core` `wasm` feature from `examples/leader/Cargo.toml`
- verified `cargo check --manifest-path examples/leader/Cargo.toml` passes on host target
- replaced the placeholder `docs/src/index.md` content with current project status text

## The Why

### Why fix example/docs drift during Phase 0 instead of deferring it

The roadmap is being used as an execution queue. If examples continue to reference APIs that no longer exist, they create false confidence and slow future work. Fixing obvious drift now makes the codebase a more trustworthy foundation for upcoming LAN/runtime changes.

### Why host-target check for `examples/leader` is still useful

The example’s library code is wasm-gated, so a host `cargo check` does not validate full wasm behavior. It still catches:

- manifest dependency/feature drift (which it did)
- server-side example compilation
- lockfile and dependency resolution issues

This was enough to produce a real improvement in the current slice while keeping scope bounded.

## Blockers & Solutions

### Blocker: example depended on missing `arbiter-core` feature `wasm`

- Problem: `cargo check --manifest-path examples/leader/Cargo.toml` failed before compilation because `arbiter-core` has no `wasm` feature.
- Solution: removed the nonexistent feature flag from the example dependency declaration and regenerated the example lockfile via `cargo check`.

### Blocker: stale API imports and trait impl shape

- Problem: `examples/leader/src/lib.rs` referenced a nonexistent `runtime::Runtime` item and used legacy empty `LifeCycle` impls.
- Solution: removed the stale import and updated `LifeCycle` impls to define `StartMessage`/`StopMessage` and lifecycle hooks.

## Fallback Plan

If the example continues to drift faster than core APIs stabilize:

1. Keep a minimal “known stale” inventory in `notes/active/...`.
2. Gate unsupported example paths clearly in docs.
3. Prioritize a smaller canonical example that is part of workspace CI while leaving the richer wasm example as best-effort until Phase 4 polish.

## Validation Performed

- `cargo check --manifest-path examples/leader/Cargo.toml` (passed, host target)
- `just lint` (passed)
- `just test` (passed)

## Remaining Gaps

- `examples/leader` wasm-target compatibility is not yet fully revalidated in this cycle
- more docs/example API drift may remain outside the touched files

