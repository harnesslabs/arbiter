# Arbiter

`arbiter` is a Rust-based multi-agent framework for event-driven simulations and coordinated agent systems.

The project is currently under active refactor and expansion:

- `arbiter-core` contains the core agent, handler, and network abstractions
- `arbiter` re-exports the public surface while internals evolve
- current roadmap tracking lives in `notes/roadmap.md` (repo-local engineering memory)

Near-term priorities:

- core contract cleanup for distributed-capable messaging
- LAN-capable brokered runtime support
- observability and replay tooling
