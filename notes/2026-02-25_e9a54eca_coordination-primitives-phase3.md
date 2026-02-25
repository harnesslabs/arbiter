---
date: 2026-02-25
commit: e9a54eca
status: completed
tags:
  - milestone
  - phase3
  - coordination
  - broker
  - node-runtime
components:
  - arbiter-core/src/protocol.rs
  - arbiter-core/src/runtime/broker.rs
  - arbiter-core/src/runtime/node.rs
  - arbiter-core/src/runtime/supervision.rs
  - arbiter-core/src/runtime/timers.rs
---

# Phase 3 Coordination Primitives

## Why

- We already had LAN transport/routing and observability, but users still had to assemble common coordination patterns manually.
- The roadmap called for practical coordination APIs before more ecosystem polish; group routing, registry lookup, request/reply, supervision, and timers are the minimum set that makes the LAN runtime feel usable.
- Keeping this work inside `arbiter-core` preserves the core-first packaging strategy and avoids premature crate decomposition.

## What Shipped

- Broker/node registry lookup helpers using existing `AgentId` advertisement state (`LookupAgent`, `LookupAgentResult`, `NodeClient::lookup_agent`).
- Group/topic membership and broker fanout routing via `GroupJoin`, `GroupLeave`, `GroupAck`, and `Recipient::Group`.
- Correlation-based request/reply helper with timeout on `NodeClient` (`request_envelope_with_timeout`).
- `runtime::supervision` with restart/retry policies (`Immediate`, `FixedDelay`, `ExponentialBackoff`) and a stateful `Supervisor`.
- `runtime::timers` with one-shot and interval scheduling helpers for node-local periodic work.
- Coordination capability negotiation hooks:
  - protocol capability constants
  - handshake capability builder/check helpers
  - broker config helper to advertise coordination capabilities
  - node client helper to inspect broker-supported capabilities
- Integration tests for group routing, registry lookup, request/reply success + timeout, and capability propagation.

## Blockers & Solutions

### Request/reply helper timed out despite a matching reply

- **Failure:** The new request/reply test timed out even though a responder sent a properly correlated reply.
- **Cause:** `request_envelope_with_timeout` pushed unrelated frames into `pending_frames`, then immediately read from `recv_frame()` again, which prioritizes `pending_frames`. This caused the same unrelated frame to be re-read and re-queued forever (starvation loop).
- **Solution:** Scan buffered frames once up front, then wait on a transport-only receive path (`recv_required_transport_frame`) while preserving non-matching frames in the pending queue.

### Intermittent `frame is too large` errors during failing test runs

- **Failure:** Broker logged frame-length corruption values matching JSON prefixes (`{\"Ad`, `{\"Gr`).
- **Cause:** This surfaced while the request/reply test was failing and panicking, which left spawned responder/broker tasks in an inconsistent state during concurrent test execution. The corruption symptom disappeared once the request/reply starvation bug was fixed and the tests shut down cleanly.
- **Solution:** Fix the request/reply starvation bug, rerun the full `arbiter-core` suite, and confirm stable green runs.

## Fallback Plan

- If group/topic routing proves too coupled to the current broker state layout, revert to addressed+broadcast only and keep group frames behind a feature gate while preserving the `NodeClient` APIs as stubs returning `UnexpectedFrame`.
- If the request/reply helper becomes too opinionated for mixed workloads, keep the low-level `send_envelope`/`recv_frame` APIs and move correlation matching into a separate helper struct instead of `NodeClient`.
- If supervision/timer APIs need redesign, they can be isolated to `runtime::supervision` and `runtime::timers` without affecting the wire protocol or broker runtime.

## Validation

- `cargo test -p arbiter-core --all-features`
- `just lint`
- `just test`
