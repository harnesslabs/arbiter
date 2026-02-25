---
date: 2026-02-25
commit: pending
status: in_progress
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

- Create `arbiter_core::observe` module and event types
- Instrument broker and node runtime paths with event emission hooks
- Add a simple in-memory recorder and JSONL file sink
- Add replay schema + recorded-order playback over observed envelope events

## Risks / Design Notes

- Observability should not block the broker hot path by default; use non-blocking channels or best-effort sinks.
- Replay is recorded-order replay, not full distributed determinism.
- Avoid entangling event schema with transport internals so local runtimes can emit the same event types later.

