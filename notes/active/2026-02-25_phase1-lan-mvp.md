---
date: 2026-02-25
commit: 8ed4fe4c
status: in_progress
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

## Next Slice (Immediate Follow-On)

- Add broker runtime + node runtime modules on top of `FramedTcpStream`
- Route addressed and broadcast `WireEnvelope`s through a broker connection registry
- Add heartbeat/session liveness checks and disconnect handling
- Define bounded queue/backpressure behavior for broker routing

## Risks / Design Notes

- The new TCP transport is intentionally separate from the legacy `Network` trait; the existing `Network` abstraction is not yet a good fit for brokered LAN sessions.
- Frames are JSON-encoded for debuggability; binary/alternative codecs can be layered later using `CodecKind`/`Codec`.
- `ProtocolVersion::matches` currently requires exact version match (major+minor), which is conservative for the early LAN milestone.

## Validation Status

- `cargo test -p arbiter-core --all-features`: passed
- `just lint`: passed
- `just test`: passed
