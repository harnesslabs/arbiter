---
date: 2026-02-25
commit: pending
status: in_progress
tags:
  - active
  - phase3
  - coordination
  - registry
  - request-reply
components:
  - arbiter-core/src/runtime/broker.rs
  - arbiter-core/src/runtime/node.rs
  - arbiter-core/src/protocol.rs
---

# Phase 3: Coordination Primitives

## Scope for Current Cycle

- Add broker-backed service/agent registry helpers
- Add groups/topics routing support
- Add request/reply helpers with timeout on top of node/broker runtime
- Add supervision and timer primitives (initial versions)

## Starting Context

- Phase 1 LAN MVP complete (brokered TCP runtime + routing + liveness + backpressure counters)
- Phase 2 complete (observe/replay instrumentation and sinks)
- Broker already supports `AgentId` advertisement and addressed/broadcast routing

## Next Slice (Immediate Follow-On)

- Extend protocol and broker runtime for group/topic membership and routing
- Add node helper APIs for registry advertisement / lookup convenience
- Add request/reply helper with correlation IDs + timeout semantics
- Instrument new coordination flows using existing `observe` events

## Risks / Design Notes

- Avoid overfitting registry/group semantics into one frame type; keep protocol messages small and composable.
- Prefer explicit broker-mediated coordination messages over hidden side effects.
- Reuse existing correlation metadata and observability events to make request/reply timeouts debuggable.

