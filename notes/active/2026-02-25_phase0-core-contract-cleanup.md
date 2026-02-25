---
date: 2026-02-25
commit: ae1f9925
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
- Introduced `arbiter_core::runtime` with `runtime::in_memory::spawn(...)`
- Moved in-memory processing loop out of `Agent::<_, InMemory>::process()` into runtime module (delegation now in `Agent::process`)
- Started docs/example alignment pass:
- Fixed stale `runtime::Runtime` import in `examples/leader/src/lib.rs`
- Updated `examples/leader` `LifeCycle` impls to current trait shape (`StartMessage`/`StopMessage`)
- Removed nonexistent `arbiter-core` `wasm` feature from `examples/leader/Cargo.toml`
- `cargo check --manifest-path examples/leader/Cargo.toml` now passes on host target

## Next Slice (Immediate Follow-On)

- Start docs/example API alignment pass (remove stale `runtime::Runtime` references)
- Continue runtime modularization toward broker/node runtimes (shared dispatch/control utilities)
- Audit remaining `examples/leader` API drift for wasm-target build compatibility

## Risks / Design Notes

- `MessageKind` currently defaults to Rust `type_name::<T>()`, which is convenient but may be unstable across refactors. Remote APIs should allow explicit user-specified kinds.
- `Recipient::Group` is modeled but not yet implemented in in-memory routing.
- Handler registration now maintains a stable wire registry (`MessageKind` + `SchemaVersion` -> local handler) and can dispatch without a usable `TypeId`, but transport/runtime layers are still local-only.
- Runtime extraction is currently organizational (module boundary + delegation). It is not yet a generic runtime abstraction for TCP/broker execution.

## Validation Status

- `cargo test -p arbiter-core`: passed
- `just lint`: passed
- `just test`: passed
- Latest validation after wire-dispatch fallback slice: `just lint` + `just test` passed
- Latest validation after runtime-module extraction slice: `just lint` + `just test` passed
- Latest validation after docs/example alignment slice: `just lint` + `just test` passed
- Extra validation: `cargo check --manifest-path examples/leader/Cargo.toml` passed
