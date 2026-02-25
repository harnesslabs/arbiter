---
date: 2026-02-25
commit: c269002f
status: completed
tags:
  - active
  - phase2
  - observability
  - replay
components:
  - arbiter-core/src/observe.rs
  - arbiter-core/src/replay.rs
  - arbiter-core/src/runtime/broker.rs
  - arbiter-core/src/runtime/node.rs
---

# Phase 2: Observability + Replay

## Scope for Current Cycle

- Add structured observability events for transport/broker/node message lifecycle
- Integrate event sinks (initial in-memory + file sink)
- Capture correlation metadata in emitted events
- Start a replay log schema and recorded-order replay harness

## Starting Context

- Phase 0 complete: distributed-capable envelope metadata, stable wire dispatch fallback, runtime extraction, docs/example alignment
- Phase 1 complete: framed TCP transport, handshake, brokered LAN runtime MVP, addressed/broadcast routing, heartbeat timeout, bounded queues
- Existing protocol metadata already includes `message_id` + optional `correlation_id`, which Phase 2 should surface in logs/events

## Next Slice (Immediate Follow-On)

- Phase 2 is complete in roadmap tracking
- Start Phase 3: coordination primitives (registry, groups/topics, request/reply, supervision, timers)
- Leverage observability events for coordination diagnostics and timeout behavior

## Risks / Design Notes

- Observability should not block the broker hot path by default; use non-blocking channels or best-effort sinks.
- Replay is recorded-order replay, not full distributed determinism.
- Avoid entangling event schema with transport internals so local runtimes can emit the same event types later.

## Completed in This Cycle

- Added `arbiter_core::observe` module with structured event schema and envelope summaries
- Added `Observer` dispatcher plus sinks:
  - `InMemoryRecorder`
  - `JsonlFileSink`
  - `TracingSink`
- Added `arbiter_core::replay` module behind `replay` feature with:
  - JSONL replay log read/write
  - recorded-order replay harness
  - explicit mismatch errors
- Instrumented `runtime::broker` and `runtime::node` with event emission hooks
- Added e2e broker observability test verifying correlation metadata propagation into routed events
- Added unit tests for observe JSONL sink and replay roundtrip/mismatch handling

## Validation Status

- `cargo test -p arbiter-core --all-features`: passed
- `just lint`: passed
- `just test`: passed
