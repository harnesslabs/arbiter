---
date: 2026-02-25
commit: 8ed4fe4c
tags:
  - phase1
  - arbiter-core
  - tcp
  - transport
  - handshake
related_components:
  - arbiter-core/src/network/tcp.rs
  - arbiter-core/src/protocol.rs
  - arbiter-core/Cargo.toml
  - notes/active/2026-02-25_phase1-lan-mvp.md
---

# Phase 1 Milestone: Framed TCP Transport + Versioned Handshake

## Summary

This cycle establishes the LAN transport baseline inside `arbiter-core`:

- replaces the placeholder TCP stub with a functional framed transport (`FramedTcpStream`)
- adds length-delimited JSON `WireFrame` send/receive
- adds client/server handshake helpers with explicit protocol version + codec negotiation
- adds transport-level protocol types to `arbiter_core::protocol`
- adds transport tests for framing and handshake rejection paths

This closes the first three Phase 1 roadmap milestones in one slice:

- functional TCP transport
- framing (length-delimited)
- versioned handshake (protocol, node id, codec/capabilities)

## The Why

### Why build transport + handshake before broker runtime

Broker/node runtime code depends on clear connection semantics:

- how bytes become frames
- how a connection is validated
- how version/codec mismatches fail

If broker routing had been built first, transport concerns would have leaked into broker logic and made tests harder to isolate. This slice gives a stable transport foundation the broker can consume.

### Why `FramedTcpStream` is separate from the current `Network` trait

The current `Network` trait models a local agent-facing abstraction (`new`, `join`, `send`, `receive`) and assumes a much simpler topology. Brokered LAN sessions need:

- socket lifecycle and listener/accept flows
- framed I/O
- handshake sequencing
- connection-level errors

Forcing this into the existing trait now would create awkward APIs and slow progress. The transport can be integrated with higher-level runtimes first, and the trait boundary can be revisited later.

### Why JSON framing first

JSON is slower than a binary codec, but for early LAN MVP work it is materially easier to debug and inspect during iteration. This cycle still introduced `CodecKind` and a `Codec` trait so alternative codecs can be added later without redesigning the wire frame model.

## Blockers & Solutions

### Blocker: `tokio` workspace features were insufficient for TCP framing

- Problem: `arbiter-core` only inherited `tokio` features for sync/runtime/macros/time, not `net`/`io-util`.
- Solution: enabled `net` and `io-util` for `arbiter-core` (and dev-deps) and added `thiserror` for structured transport errors.

### Blocker: handshake failures needed to be explicit on both client and server sides

- Problem: version/codec mismatch should be visible both to the server (validation error) and client (reject frame), not silently close the socket.
- Solution: server handshake helper sends a `HelloReject` best-effort frame before returning a typed transport error; client handshake maps `HelloReject` into `TcpTransportError::HandshakeRejected`.

## Fallback Plan

If the framed transport layer becomes a bottleneck or the API proves wrong:

1. Keep protocol types in `protocol.rs` (they are broadly useful).
2. Replace `FramedTcpStream` internals while preserving `send_frame`/`recv_frame`/handshake helper signatures.
3. If necessary, move handshake helpers out of the transport type into free functions, keeping tests as the contract.

This limits churn for upcoming broker runtime work.

## Validation Performed

- `cargo test -p arbiter-core --all-features` (passed)
- `cargo fmt --all` (ran successfully; nightly-config warnings only)
- `just lint` (passed)
- `just test` (passed)

## Follow-On Work (Next Slice)

- Broker runtime module with connection registry
- Addressed/broadcast routing across connected nodes
- Heartbeats and disconnect handling
- Bounded queue/backpressure policy

