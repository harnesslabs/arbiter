# Compatibility & Features

## Protocol Compatibility (Current)

- transport protocol version: `0.1` (`ProtocolVersion::current()`)
- handshake enforces exact `major.minor` match today
- codec negotiation is pluggable; JSON (`CodecKind::Json`) is the default built-in codec

## Feature Flags

### `arbiter` façade crate

- `in-memory` (default): local/in-memory runtime support
- `tcp`: LAN transport and broker/node runtime modules
- `replay`: replay module (depends on event logs produced by `observe`)
- `fixtures`: testing/fixture helpers

### `arbiter-core`

`arbiter` forwards these feature flags directly to `arbiter-core` so callers can use a stable import path while internals evolve.

## Coordination Capability Negotiation

Coordination features advertise handshake capabilities (strings) so nodes can check broker support before using optional flows.

Current built-in capability constants live in:

- `arbiter::protocol::capabilities::GROUP_ROUTING_V1`
- `arbiter::protocol::capabilities::AGENT_LOOKUP_V1`
- `arbiter::protocol::capabilities::REQUEST_REPLY_V1`
- `arbiter::protocol::capabilities::SUPERVISION_V1`
- `arbiter::protocol::capabilities::TIMERS_V1`

## Crate Extraction Review (Current Decision)

Current decision: **do not split additional crates yet**.

Rationale:

- `arbiter-core` still benefits from fast iteration across protocol/runtime/observe/replay/coordinator changes
- compile time and dependency weight are not yet forcing isolation
- protocol consumers are currently Rust-only, so independent semver for a protocol crate is premature

## Triggers for Future Crate Splits

- protocol needs independent semver for non-Rust/external consumers
- optional subsystems add heavy dependencies or materially increase default compile times
- subsystems need independent release cadence
- separation clearly improves public API usability
- build/test time becomes a sustained productivity bottleneck
