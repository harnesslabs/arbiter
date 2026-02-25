---
date: 2026-02-25
commit: 877d0822
status: in_progress
tags:
  - active
  - phase0
  - arbiter-core
  - protocol
components:
  - arbiter-core/src/handler.rs
  - arbiter-core/src/agent.rs
  - arbiter-core/src/protocol.rs
---

# Phase 0: Core Contract Cleanup

## Scope for Current Cycle

- Seed stable protocol metadata types in `arbiter-core`
- Extend envelopes with routing/correlation metadata
- Add addressed-vs-broadcast behavior to in-memory processing
- Replace panic-on-decode with structured handler error
- Preserve existing in-memory behavior and tests

## Completed in This Cycle

- Added `arbiter_core::protocol` module with `MessageKind`, `SchemaVersion`, IDs, `Recipient`, `EnvelopeMeta`
- Added `Envelope` metadata + builder helpers (`to_address`, `to_agent`, `broadcast`, sender/correlation/schema setters)
- Updated in-memory processing loop to filter on `Recipient`
- Added structured `HandlerError` and removed panic on payload decode mismatch
- Added tests for addressed delivery and structured decode error
- Added stable kind/version handler registration + dispatch fallback (`TypeId` fallback to `MessageKind` + `SchemaVersion`)
- Added tests proving dispatch works even when envelope `TypeId` is unusable

## Next Slice (Immediate Follow-On)

- Decouple runtime loop from `InMemory` specialization into shared runtime abstractions
- Start docs/example API alignment pass (remove stale `runtime::Runtime` references)

## Risks / Design Notes

- `MessageKind` currently defaults to Rust `type_name::<T>()`, which is convenient but may be unstable across refactors. Remote APIs should allow explicit user-specified kinds.
- `Recipient::Group` is modeled but not yet implemented in in-memory routing.
- Handler registration now maintains a stable wire registry (`MessageKind` + `SchemaVersion` -> local handler) and can dispatch without a usable `TypeId`, but transport/runtime layers are still local-only.

## Validation Status

- `cargo test -p arbiter-core`: passed
- `just lint`: passed
- `just test`: passed
- Latest validation after wire-dispatch fallback slice: `just lint` + `just test` passed
