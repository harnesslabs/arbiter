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

To run the example locally, navigate to `examples/leader` and follow the `README.md` instructions there, or run the build directly using your preferred WASM server.
