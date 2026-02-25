# Local Simulation Guide

This guide uses the in-memory runtime path in `arbiter-core` for fast iteration and unit testing.

## What You Get

- typed handlers (`Handler<M>`)
- envelope metadata (`EnvelopeMeta`) for addressed/broadcast semantics
- no TCP/network setup required

## Enable the Right Feature

The default `arbiter` feature set includes `in-memory`, so a plain dependency is enough:

```toml
[dependencies]
arbiter = "0.5"
```

## Recommended Development Loop

```sh
just lint
just test
```

## Key Concepts to Use

- `arbiter::prelude::*` for handlers/messages and runtime helper exports
- `arbiter::protocol::Recipient` for `Broadcast` vs addressed delivery
- `arbiter::protocol::EnvelopeMeta` when you need explicit metadata (correlation, recipients, schema version)

## Addressed vs Broadcast (Conceptual)

Use addressed delivery when a message targets a specific `AgentId`; use broadcast for fanout to all local recipients.

```rust,ignore
use arbiter::protocol::EnvelopeMeta;

let addressed = EnvelopeMeta::new("example.request").to_agent("agent-a");
let broadcast = EnvelopeMeta::new("example.broadcast").broadcast();
```

## Validation

The repository already includes in-memory integration coverage in:

- `arbiter-core/src/agent.rs`
- `arbiter-core/tests/multi_agent.rs`
