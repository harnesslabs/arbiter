# Core Concepts

Arbiter's design hinges on a few central abstractions that make setting up complex, event-driven interactions straightforward and efficient.

## Actor

At the heart of Arbiter is the **Actor**. An actor encapsulates state and behavior. It is the fundamental unit of computation that communicates with other actors exclusively through message passing.

Actors in Arbiter are defined by a standard Rust `struct` that implements a few key traits.

## LifeCycle

The `LifeCycle` trait defines the core temporal stages of an actor's existence:

- `on_start()`: Called when the actor is spawned into the runtime. Ideal for initialization tasks or broadcasting startup events.
- `on_stop()`: Called during shutdown. Use this for cleanup and persistence.
- `snapshot()`: Produces a snapshot of the actor's current state, which is incredibly useful for simulation logging, testing, or building replays.

## Handler

While `LifeCycle` defines how an actor lives and dies, the `Handler<M>` trait defines how it interacts with the world. You implement `Handler<M>` for each message type `M` your actor needs to understand.

```rust
// An example of an actor handling a specific `Tick` message.
impl Handler<Tick> for MyActor {
    type Reply = Update;

    fn handle(&mut self, message: &Tick) -> Option<Self::Reply> {
        // Business logic here
        Some(Update { ... })
    }
}
```

The `Network` manages connections between actors and the broader system. It handles the low-level details of broadcasting messages and establishing direct communication channels (Sockets). 

Arbiter provides two main implementations:
- **`InMemory`**: High-performance, same-process communication using standard Rust channels.
- **`TcpStream`**: Distributed communication using TCP, enabling actors to span multiple processes and nodes.

This abstraction allows Arbiter systems to scale from local in-memory runners to distributed processing nodes.

## Runtime

The `Runtime` provides the execution backdrop. It schedules the actors, mediates their inbox subscriptions, manages the network, and ensures that the system ticks along efficiently. When an actor is placed in a runtime, it is granted a `Socket` that it uses for receiving and emitting messages.
