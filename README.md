<img width="529" alt="arbiter" src="https://user-images.githubusercontent.com/20118821/236929861-2a1fe071-0053-453c-ac86-224b32febcd6.png">

> A Rust-based multi-agent framework.

<p align="center">
  <a href="https://discord.gg/qEwPr3GMP2"><img src="https://img.shields.io/discord/1363153354338861247?style=flat&logo=discord&logoColor=white&label=Discord&color=5865F2" alt="Discord"></a>
  <a href="https://x.com/harnesslabs"><img src="https://badgen.net/badge/icon/twitter?icon=twitter&label" alt="Twitter"></a>
  <img src="https://visitor-badge.laobi.icu/badge?page_id=arbiter" alt="Visitors">
</p>

## Overview

> **Arbiter** is a Rust-based, event-driven multi-agent framework that lets developers orchestrate strongly-typed, high-performance simulations and networked systems.

Arbiter provides the foundational types and traits for building actor-based systems with pluggable networking and lifecycle management. It is designed around a lightweight actor model tailored for discrete-event simulation, automated trading, and complex distributed systems.

---

## Core Concepts

Arbiter's architecture is built around several key traits and structs that make agent development straightforward:

- **`Actor`**: The core execution unit. Contains the agent's internal state and logic.
- **`LifeCycle`**: A trait defining the start, stop, and snapshot behavior of an actor.
- **`Handler<M>`**: A trait used to implement message-handling logic for specific message types.
- **`Network`**: A generalized trait for instantiating and managing connections between parts of the system.
- **`Runtime`**: Manages the execution context of the actors, mediating subscriptions and routing messages.

## Quick Example

Here is a glimpse of how you can build a simple actor with Arbiter that handles `Ping` messages:

```rust
use arbiter::prelude::*;

#[derive(Debug, Clone)]
pub struct Ping;

#[derive(Debug, Clone)]
pub struct Counter {
    pub count: usize,
}

// Define the actor's lifecycle
impl LifeCycle for Counter {
    type Snapshot = usize;
    type StartMessage = ();
    type StopMessage = ();

    fn on_start(&mut self) -> Self::StartMessage {}
    fn on_stop(&mut self) -> Self::StopMessage {}
    fn snapshot(&self) -> Self::Snapshot { self.count }
}

// Implement message passing logic
impl Handler<Ping> for Counter {
    type Reply = ();

    fn handle(&mut self, _message: &Ping) -> Option<Self::Reply> {
        self.count += 1;
        println!("Counter received Ping. Total: {}", self.count);
        None
    }
}
```

## Documentation

For an in-depth dive into Arbiter's design, extensive examples, and API specifics, please check out our [MDBook Documentation](https://arbiter.harnesslabs.dev).

## Contributing

We welcome community contributions! Please see our [Contributing Guidelines](https://github.com/harnesslabs/arbiter/blob/main/.github/CONTRIBUTING.md) to get started.
