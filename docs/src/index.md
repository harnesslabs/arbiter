# Arbiter

`arbiter` is a Rust-based multi-agent framework for event-driven simulations and coordinated agent systems.

Current project shape (core-first roadmap execution):

- `arbiter-core` contains the core runtime, protocol, transport, observability, replay, and coordination primitives
- `arbiter` is the façade crate with feature passthroughs and re-exports for stable imports
- roadmap execution and engineering memory live in `notes/roadmap.md` and `notes/`

Implemented milestones:

- distributed-capable core message metadata and routing semantics
- brokered LAN runtime (TCP framing, handshake, routing, liveness, backpressure)
- observability + replay support
- coordination primitives (group routing, registry lookup, request/reply, supervision policies, timers)

Start here:

- `Local Simulation Guide` for in-memory agent simulation
- `LAN Runtime Guide` for broker/node networking on a trusted LAN
- `Replay Guide` for structured event capture and recorded-order replay
- `Compatibility & Features` for protocol versioning, feature flags, and crate-split policy
