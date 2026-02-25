# LAN Runtime Guide

This guide covers the brokered LAN topology currently implemented in `arbiter-core`.

## Scope

- trusted LAN only (no TLS/auth yet)
- brokered mesh topology (no P2P)
- static bootstrap (explicit broker address)

## Features

Enable `tcp` on the façade crate:

```toml
[dependencies]
arbiter = { version = "0.5", features = ["tcp"] }
```

## Broker + Node Client (Minimal Flow)

```rust,ignore
use std::net::SocketAddr;
use arbiter::protocol::{AgentId, HandshakeHello};
use arbiter::runtime::broker::{self, BrokerConfig};
use arbiter::runtime::node::NodeClient;

let broker = broker::spawn(
    BrokerConfig::new(SocketAddr::from(([127, 0, 0, 1], 0)), "broker-1")
        .with_coordination_capabilities(),
).await?;

let mut node = NodeClient::connect(
    broker.local_addr(),
    HandshakeHello::new("node-a").with_coordination_capabilities(),
).await?;

node.advertise_agents(vec![AgentId::from("agent-a")]).await?;
```

## Coordination Primitives Available

- group/topic fanout: `join_groups`, `leave_groups`, `Recipient::Group`
- registry lookup: `lookup_agent`
- request/reply helper: `request_envelope_with_timeout`
- capability checks: `broker_supports_capability(...)`

## Example Validation Commands

```sh
just check-examples
cargo test -p arbiter-core --all-features runtime::broker
```

## Notes

- LAN routing is message-level (`WireEnvelope`) and uses protocol metadata, not Rust `TypeId`.
- Heartbeat timeout and bounded outbound queues are handled by the broker runtime.
