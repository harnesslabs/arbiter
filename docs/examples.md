# Examples

Learning by example is one of the best ways to get familiar with Arbiter.

## Ping, Pong!

The classic Actor model example: Ping Pong. We want an actor that acts as a `Counter` and responds to `Ping` messages.

```rust
use arbiter::prelude::*;

#[derive(Debug, Clone)]
pub struct Ping;

#[derive(Debug, Clone)]
pub struct Counter {
    pub count: usize,
}

impl LifeCycle for Counter {
    type Snapshot = usize;
    type StartMessage = ();
    type StopMessage = ();

    fn on_start(&mut self) -> Self::StartMessage {}
    fn on_stop(&mut self) -> Self::StopMessage {}
    fn snapshot(&self) -> Self::Snapshot { self.count }
}

impl Handler<Ping> for Counter {
    type Reply = (); // We aren't replying to the ping here

    fn handle(&mut self, _message: &Ping) -> Option<Self::Reply> {
        self.count += 1;
        println!("Received Ping! Count: {}", self.count);
        None
    }
}
```

## The WASM Leader / Follower Example

Arbiter supports compiling to WebAssembly (`wasm32-unknown-unknown`). In the `examples/leader/` directory, you'll find a complete web application utilizing Arbiter actors to control UI components on an HTML canvas.

The example demonstrates:
- A `Leader` actor that moves autonomously.
- `Follower` actors that track and chase the leader.
- An overarching `Canvas` actor the manages rendering by interpreting the `PositionUpdate` messages emitted by agents.

### Running the Leader Example

To run the example locally, navigate to `examples/leader` and follow the `README.md` instructions there, or run the build directly using your preferred WASM server.

## The TCP Chat CLI Example

Arbiter's `TcpStream` network allows actors to communicate over a local network. The `examples/chat/` directory contains a decentralized chat application that demonstrates:

- **Distributed Actors**: Actors on different nodes connecting and exchanging messages.
- **Dynamic Registration**: Automatic type registration for complex serializable types across nodes.
- **Socket Injection**: Using `Runtime::socket()` to bridge external input (keyboard/stdin) into the actor network.

### Running the Chat Example

1. Open a terminal and start the first node (Alice):
   ```bash
   cargo run --example chat -- --name Alice
   ```
2. Open another terminal and start the second node (Bob), connecting to Alice's address (shown in Alice's terminal):
   ```bash
   cargo run --example chat -- --name Bob --connect 127.0.0.1:<PORT>
   ```
3. Type messages in either terminal to see them delivered in real-time across processes!

