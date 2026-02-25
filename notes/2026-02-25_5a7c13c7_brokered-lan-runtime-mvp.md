---
date: 2026-02-25
commit: 5a7c13c7
tags:
  - phase1
  - arbiter-core
  - broker
  - lan
  - runtime
related_components:
  - arbiter-core/src/runtime/broker.rs
  - arbiter-core/src/runtime/node.rs
  - arbiter-core/src/protocol.rs
  - notes/active/2026-02-25_phase1-lan-mvp.md
---

# Phase 1 Milestone: Brokered LAN Runtime MVP

## Summary

This cycle completes the remaining Phase 1 LAN MVP milestones:

- broker runtime module (`runtime::broker`)
- node client runtime module (`runtime::node`)
- addressed and broadcast routing across TCP-connected nodes
- heartbeat timeout + disconnect cleanup
- bounded outbound queue semantics with explicit drop counters

The broker runs on top of the framed TCP transport introduced in the previous cycle and uses node-side `AdvertiseAgents` registration to route addressed messages by `AgentId`.

## The Why

### Why use explicit `AdvertiseAgents` instead of implicit routing

The wire envelope recipient model already targets `AgentId`, but the broker does not inherently know which node hosts which agents. Introducing explicit advertisement frames keeps the routing logic simple and explicit without prematurely designing full discovery/service registry APIs (which are Phase 3 work).

### Why bounded queues + `try_send` drop counters for MVP

The broker needs a backpressure policy now, even before observability/replay is built. The chosen MVP behavior is:

- bounded per-peer outbound queues
- non-blocking routing (`try_send`)
- count drops caused by backpressure

This avoids head-of-line blocking inside broker connection tasks and gives us measurable behavior to instrument in Phase 2.

### Why heartbeat timeout is connection-local

Heartbeat/liveness is implemented in each connection task using the broker registry’s last-seen timestamp. This keeps the implementation localized and easy to test before adding centralized supervision or monitoring.

## Blockers & Solutions

### Blocker: routing addressed messages required node ownership data

- Problem: envelopes target `AgentId`, but the broker initially only knew about node connections.
- Solution: added protocol frames `AdvertiseAgents` and `AdvertiseAck`, and broker-side `agent_routes` mapping.

### Blocker: concurrent reads/writes on one TCP connection

- Problem: `FramedTcpStream` currently owns an unsplit `TcpStream`, so independent read/write tasks would complicate borrowing and splitting.
- Solution: each broker connection uses a single `tokio::select!` loop that multiplexes inbound frames, outbound queue sends, and heartbeat checks.

### Blocker: deterministic backpressure testing via network I/O is hard

- Problem: OS socket buffering makes end-to-end queue saturation timing non-deterministic.
- Solution: added a direct unit test over `BrokerState` using a bounded `mpsc` channel to prove counter behavior when `try_send` hits a full queue.

## Fallback Plan

If this broker MVP architecture proves too rigid:

1. Keep `runtime::node` and protocol advertisement frames (they are still useful).
2. Replace broker routing internals while preserving `BrokerConfig` / `BrokerHandle` external shape as much as possible.
3. If needed, move from per-connection `select!` loops to split read/write tasks once transport abstractions mature.

This preserves the usable LAN API while allowing internal broker rewrites.

## Validation Performed

- `cargo test -p arbiter-core --all-features` (passed)
- `cargo fmt --all` (ran successfully; nightly-config warnings only)
- `just lint` (passed)
- `just test` (passed)

## Follow-On Work (Next Slice)

- Phase 2 observability module and event schema
- Correlation metadata propagation in broker/node transport events
- Replay log format and recorded-order replay harness

