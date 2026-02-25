---
date: 2026-02-25
commit: c269002f
tags:
  - phase2
  - arbiter-core
  - observability
  - replay
  - tracing
related_components:
  - arbiter-core/src/observe.rs
  - arbiter-core/src/replay.rs
  - arbiter-core/src/runtime/broker.rs
  - arbiter-core/src/runtime/node.rs
  - notes/active/2026-02-25_phase2-observability-replay.md
---

# Phase 2 Milestone: Observability + Replay Support

## Summary

This cycle completes the Phase 2 roadmap:

- structured observability event schema (`observe` module)
- correlation metadata surfaced end-to-end in emitted events
- file sink + `tracing` sink integration
- feature-gated replay module with JSONL replay logs and recorded-order replay harness
- broker/node runtime instrumentation and tests

## The Why

### Why add observability before more coordination APIs

The new LAN broker runtime and transport work introduced many state transitions and failure modes (handshakes, routing, disconnects, backpressure). Building more coordination primitives first would make debugging much harder. Phase 2 gives us shared visibility primitives before the API surface expands.

### Why use an event schema + sinks instead of direct logging everywhere

Direct logging ties runtime behavior to one output channel and makes replay difficult. A structured event schema lets us:

- capture broker/node state transitions consistently
- route the same event to in-memory, file, and tracing outputs
- reuse the exact JSONL event format as replay input

### Why replay is recorded-order only

Full distributed determinism is a much larger problem. Recorded-order replay is immediately useful for debugging and regression checks and aligns with the current roadmap without overpromising semantics we cannot guarantee yet.

## Blockers & Solutions

### Blocker: observability needed to include envelope correlation metadata, not just frame kinds

- Problem: frame-level logs alone are insufficient for tracing message flows.
- Solution: added `EnvelopeSummary` to observability events and instrumented broker/node paths so correlation/message IDs are preserved in emitted events. Added an end-to-end test that verifies this.

### Blocker: sink abstractions needed to stay lightweight

- Problem: introducing a complex async observability pipeline would slow down Phase 2.
- Solution: implemented a simple synchronous `Observer` + sink fanout with best-effort sinks and documented that a non-blocking pipeline can be layered later if needed.

## Fallback Plan

If the current observer/sink design becomes a bottleneck:

1. Keep `ObserveEvent` and `EnvelopeSummary` schema stable.
2. Replace `Observer` internals with an async channel-based dispatcher.
3. Preserve sink APIs (`EventSink`) where possible so broker/node instrumentation stays unchanged.

This keeps the instrumentation call sites stable while allowing performance-focused refactors later.

## Validation Performed

- `cargo test -p arbiter-core --all-features` (passed)
- `cargo fmt --all` (ran successfully; nightly-config warnings only)
- `just lint` (passed)
- `just test` (passed)

## Follow-On Work (Next Slice)

- Phase 3 coordination primitives:
  - registry and discovery helpers
  - group/topic routing
  - request/reply helpers with timeout
  - supervision and timers

