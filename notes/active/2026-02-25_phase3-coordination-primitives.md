---
date: 2026-02-25
commit: e9a54eca
status: completed
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
- Add handshake capability negotiation hooks for coordination features

## Starting Context

- Phase 1 LAN MVP complete (brokered TCP runtime + routing + liveness + backpressure counters)
- Phase 2 complete (observe/replay instrumentation and sinks)
- Broker already supports `AgentId` advertisement and addressed/broadcast routing

## Next Slice (Immediate Follow-On)

- Phase 4 ecosystem polish: façade exports, docs/tutorial alignment, CI parity check targets, crate extraction review note

## Risks / Design Notes

- Avoid overfitting registry/group semantics into one frame type; keep protocol messages small and composable.
- Prefer explicit broker-mediated coordination messages over hidden side effects.
- Reuse existing correlation metadata and observability events to make request/reply timeouts debuggable.

## Completed Work

- Added broker/node protocol frames for `GroupJoin`, `GroupLeave`, `GroupAck`, `LookupAgent`, and `LookupAgentResult`.
- Added broker-side group membership tracking and `Recipient::Group` routing fanout.
- Added node-side `join_groups`, `leave_groups`, `lookup_agent`, and correlation-based `request_envelope_with_timeout`.
- Fixed a request/reply buffering starvation bug by scanning pending frames once and reading subsequent frames directly from transport until a matching correlation is found.
- Added coordination capability constants and handshake helpers plus broker/node capability convenience APIs.
- Added `runtime::supervision` and `runtime::timers` modules with unit tests.
- Added integration tests covering group routing, lookup, request/reply success + timeout, and capability propagation.
