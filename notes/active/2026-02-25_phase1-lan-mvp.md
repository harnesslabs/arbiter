---
date: 2026-02-25
commit: 5a7c13c7
status: completed
tags:
  - active
  - phase1
  - lan
  - tcp
  - handshake
components:
  - arbiter-core/src/network/tcp.rs
  - arbiter-core/src/protocol.rs
  - arbiter-core/Cargo.toml
---

# Phase 1: LAN MVP (Brokered Mesh, Library-First)

## Scope for Current Cycle

- Replace the `arbiter-core` TCP transport stub with a functional framed transport
- Introduce protocol handshake types for version + codec negotiation
- Add handshake helpers (client/server) and tests for success and explicit rejection paths

## Completed in This Cycle

- Replaced `arbiter-core/src/network/tcp.rs` stub with `FramedTcpStream` (length-delimited JSON `WireFrame`s)
- Added TCP listener helpers (`bind`, `accept`) and transport error types (`TcpTransportError`)
- Added `ServerHandshakeConfig` + client/server handshake helpers
- Added protocol transport primitives in `arbiter_core::protocol`:
  - `ProtocolVersion`
  - `CodecKind`
  - `Codec` trait + `JsonCodec`
  - `WireEnvelope`
  - `HandshakeHello` / `HandshakeAck` / `HandshakeReject` / `HandshakeRejectReason`
  - `Heartbeat` / `WireFrame`
- Added tests for:
  - framed transport round trip
  - handshake success
  - handshake protocol version rejection
  - handshake codec negotiation rejection
- Added `runtime::broker` and `runtime::node` LAN MVP modules on top of framed TCP transport
- Added node agent advertisement frames (`AdvertiseAgents` / `AdvertiseAck`) for addressed routing
- Added broker routing for addressed and broadcast `WireEnvelope`s across connected nodes
- Added heartbeat timeout enforcement and disconnect cleanup for peer/route registries
- Added bounded outbound queue semantics with backpressure drop counters and tests

## Next Slice (Immediate Follow-On)

- Phase 1 is complete in roadmap tracking
- Start Phase 2: observability events for broker/node/transport/message lifecycle
- Add replay log schema and recorded-order replay harness

## Risks / Design Notes

- The new TCP transport is intentionally separate from the legacy `Network` trait; the existing `Network` abstraction is not yet a good fit for brokered LAN sessions.
- Frames are JSON-encoded for debuggability; binary/alternative codecs can be layered later using `CodecKind`/`Codec`.
- `ProtocolVersion::matches` currently requires exact version match (major+minor), which is conservative for the early LAN milestone.
- Broker routing is currently based on explicit node advertisement of hosted `AgentId`s; richer service discovery and groups are deferred to later phases.

## Validation Status

- `cargo test -p arbiter-core --all-features`: passed
- `just lint`: passed
- `just test`: passed
- Broker e2e tests cover addressed routing, broadcast routing, disconnect cleanup, heartbeat timeout, and backpressure counters
