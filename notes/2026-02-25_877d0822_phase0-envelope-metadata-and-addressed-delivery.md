---
date: 2026-02-25
commit: 877d0822
tags:
  - phase0
  - arbiter-core
  - protocol
  - routing
  - error-handling
related_components:
  - arbiter-core/src/protocol.rs
  - arbiter-core/src/handler.rs
  - arbiter-core/src/agent.rs
  - notes/roadmap.md
---

# Phase 0 Milestone: Envelope Metadata + Addressed In-Memory Delivery

## Summary

This cycle implemented the first concrete Phase 0 roadmap slice inside `arbiter-core`:

- added protocol metadata primitives (`MessageKind`, `SchemaVersion`, IDs, `Recipient`, `EnvelopeMeta`)
- extended `Envelope` to carry routing/correlation metadata
- added addressed-vs-broadcast delivery filtering in the in-memory runtime loop
- replaced panic-on-decode handler dispatch with structured `HandlerError`
- added tests proving addressed delivery and structured decode errors

## The Why

### Why start with envelope metadata before TCP/broker code

LAN support cannot be implemented safely on top of the previous envelope model because messages were effectively identified by Rust `TypeId` only. `TypeId` is process-local and not a stable wire contract. Adding transport first would have produced a dead-end protocol.

This milestone makes the core message shape capable of carrying:

- stable wire identity (`MessageKind`)
- schema evolution hooks (`SchemaVersion`)
- routing intent (`Recipient`)
- correlation metadata for observability/reply linkage

These are core concerns, so they belong in `arbiter-core` for now (aligned with the "crates only when needed" roadmap).

### Why `MessageKind` currently defaults to `type_name::<T>()`

It minimizes migration friction while we introduce remote-capable dispatch gradually. Existing typed APIs continue to work, and every packaged message gets a deterministic string kind without forcing users to rewrite handlers immediately.

This is an interim default. We explicitly retained APIs (`Envelope::package_with_meta`) so later milestones can require or encourage explicit stable message kinds.

### Why addressed routing was added first only to in-memory processing

It proves the semantics (addressed vs broadcast) at the cheapest integration point and locks behavior before the TCP/broker runtime exists. It also provides a testable contract for future transport/routing implementations.

## Blockers & Solutions

### Blocker: `notes/` was ignored by `.gitignore`

- Problem: the repo-local `.codex/AGENTS.md` requires local memory in `notes/`, but `.gitignore` had `notes/` ignored.
- Solution: removed the `notes/` ignore rule so roadmap and cycle notes can be tracked.

### Blocker: handler dispatch API assumed infallible decode and panicked on mismatch

- Problem: `create_handler` panicked on payload decode mismatch, which is unacceptable for distributed/wire scenarios.
- Solution: changed handler dispatch to return `Result<HandleResult<Envelope<_>>, HandlerError>`, added structured `PayloadDecodeFailed` and `AgentTypeMismatch`.

### Blocker: introducing routing metadata risked breaking existing local tests

- Problem: adding recipient filtering could accidentally suppress existing broadcast-style in-memory behavior.
- Solution: `Recipient` defaults to `Broadcast`, and `Envelope::package(...)` uses broadcast metadata by default. Existing tests remained green, and a targeted addressed-delivery test was added.

## Fallback Plan

If this metadata-first approach proves fragile:

1. Keep `protocol.rs` and `EnvelopeMeta`, but bypass recipient filtering in `Agent::<_, InMemory>::process` (treat all messages as broadcast again).
2. Retain `HandlerError` result-returning dispatch while temporarily ignoring correlation metadata propagation.
3. Preserve the new tests and add compatibility shims so future LAN work can restart from a known, tested local baseline.

This rollback keeps the code changes mostly additive and avoids reverting the full milestone.

## Validation Performed

- `cargo test -p arbiter-core` (passed)
- `just lint` (passed)
- `just test` (passed)

## Follow-On Work (Next Slice)

- Add stable identifier-based handler registration/dispatch (`MessageKind` + `SchemaVersion`) at the wire boundary, not just metadata on the envelope.
- Begin extracting shared runtime flow out of `Agent::<_, InMemory>::process`.
- Start docs/example drift cleanup (stale runtime references).

