use std::{any::TypeId, collections::HashMap, fmt::Debug};

use crate::{
  handler::{Envelope, Handler, Message, MessageHandlerFn, create_handler},
  network::{Connection, Generateable, Network},
  processor::{Controller, Processing, State},
};

// ── LifeCycle trait ──────────────────────────────────────────────────

pub trait LifeCycle: Send + Sync + 'static {
  type StartMessage: Message + Debug;
  type StopMessage: Message + Debug;
  type Snapshot: Send + Sync + Clone + Debug + 'static;

  fn on_start(&mut self) -> Self::StartMessage;
  fn on_stop(&mut self) -> Self::StopMessage;
  fn snapshot(&self) -> Self::Snapshot;
  fn should_stop(&self) -> bool {
    false
  }
}

// ── Actor ────────────────────────────────────────────────────────────

pub struct Actor<L: LifeCycle, N: Network> {
  pub name: Option<String>,
  pub(crate) state: State,
  pub(crate) inner: L,
  pub(crate) connection: Connection<N>,
  pub(crate) handlers: HashMap<TypeId, MessageHandlerFn<N>>,
}

impl<L: LifeCycle, N: Network> Actor<L, N> {
  pub(crate) fn new(inner: L, network: &N) -> Self {
    Self {
      name: None,
      state: State::Stopped,
      inner,
      connection: Connection { address: N::Address::generate(), network: network.join() },
      handlers: HashMap::new(),
    }
  }

  pub fn with_name(mut self, name: impl Into<String>) -> Self {
    self.name = Some(name.into());
    self
  }

  pub fn set_name(&mut self, name: impl Into<String>) {
    self.name = Some(name.into());
  }

  pub fn clear_name(&mut self) {
    self.name = None;
  }

  pub fn with_handler<M>(mut self) -> Self
  where
    M: Message,
    L: Handler<M>,
  {
    self.handlers.insert(TypeId::of::<M>(), create_handler::<M, L, N>());
    self
  }

  pub fn process(self) -> Processing<Self, L, N> {
    let processing_name = self.name.clone();
    let address = self.connection.address;

    let controller = Controller::new();
    let mut inner_controller = controller.inner;
    let outer_controller = controller.outer;

    // Send the initial snapshot to observers
    let snapshot = self.inner.snapshot();
    inner_controller.snapshot_sender.send(snapshot).unwrap();

    // Destructure self so we can move individual fields into the async block.
    // This avoids the borrow checker conflict that previously required `unsafe`:
    // we need `&mut inner` and `&handlers` simultaneously, which is fine when they
    // are separate local variables, but not when accessed through `&mut self`.
    let Self { name, mut state, mut inner, mut connection, handlers } = self;

    let task = tokio::spawn(async move {
      loop {
        let prev_state = state;
        tokio::select! {
          biased;

          // ────────────────────────────────────────────────────────────────
          // Control-plane messages (START / STOP / GET_STATE)
          // ────────────────────────────────────────────────────────────────
          control_signal = inner_controller.instruction_receiver.recv() => {
            match control_signal {
              Some(crate::processor::ControlSignal::Start) => {
                state = State::Running;
                inner_controller.state_sender.send(State::Running).await.unwrap();
                let start_message = inner.on_start();
                tracing::debug!(agent = ?name, "sending start_message");
                connection.network.send(N::Envelope::wrap(start_message)).await;
              },
              Some(crate::processor::ControlSignal::Stop) => {
                state = State::Stopped;
                inner_controller.state_sender.send(State::Stopped).await.unwrap();
                let stop_message = inner.on_stop();
                connection.network.send(N::Envelope::wrap(stop_message)).await;
                break;
              },
              Some(crate::processor::ControlSignal::GetState) => {
                inner_controller.state_sender.send(prev_state).await.unwrap();
              },
              None => {
                break;
              },
            }
          }

          // ────────────────────────────────────────────────────────────────
          // Application messages coming from the transport
          // ────────────────────────────────────────────────────────────────
          message = connection.network.receive() => {
            if let Some(message) = message
              && let Some(handler) = handlers.get(&message.type_id()) {
                let reply = handler(&mut inner as &mut dyn std::any::Any, &message);

                if let Some(envelope) = reply {
                  connection.network.send(envelope).await;
                }

                let _ = inner_controller.snapshot_sender.send(inner.snapshot());

                if inner.should_stop() {
                  state = State::Stopped;
                  inner_controller.state_sender.send(State::Stopped).await.unwrap();
                  let stop_message = inner.on_stop();
                  connection.network.send(N::Envelope::wrap(stop_message)).await;
                  break;
                }
              }
          }
        }
      }

      // Reassemble self so it can be returned from the task
      Self { name, state, inner, connection, handlers }
    });

    Processing { name: processing_name, address, task, outer_controller }
  }
}

#[cfg(test)]
mod tests {
  use crate::{
    fixtures::*,
    handler::Envelope as _,
    network::memory::{InMemory, InMemoryEnvelope},
    processor::State,
    runtime::Runtime,
  };
  use tokio_stream::StreamExt;

  #[tokio::test]
  async fn lifecycle_start_stop_join() {
    let runtime = Runtime::<InMemory>::new();
    let actor = runtime.spawn(Counter { count: 0 });

    let mut processing = actor.process();
    let mut snapshots = processing.stream().unwrap();

    processing.start().await.unwrap();
    assert_eq!(processing.state().await.unwrap(), State::Running);

    // Initial snapshot emitted at process() time
    assert_eq!(snapshots.next().await.unwrap(), 0);

    processing.stop().await.unwrap();

    // join() succeeds — actor exited cleanly
    let _actor = processing.join().await.unwrap();

    // Stream closes after the actor exits
    assert_eq!(snapshots.next().await, None);
  }

  #[tokio::test]
  async fn single_handler_increments_snapshot() {
    let runtime = Runtime::<InMemory>::new();
    let actor = runtime.spawn(Counter { count: 0 }).with_handler::<Ping>();
    let sender = actor.connection.network.sender.clone();

    let mut processing = actor.process();
    let mut snapshots = processing.stream().unwrap();
    processing.start().await.unwrap();

    // Initial snapshot is 0
    assert_eq!(snapshots.next().await.unwrap(), 0);

    // Send a Ping, snapshot should become 1
    sender.send(InMemoryEnvelope::wrap(Ping)).unwrap();
    assert_eq!(snapshots.next().await.unwrap(), 1);

    processing.stop().await.unwrap();
  }

  #[tokio::test]
  async fn multiple_handlers_route_correctly() {
    let runtime = Runtime::<InMemory>::new();
    let actor = runtime.spawn(Counter { count: 0 }).with_handler::<Ping>().with_handler::<Pong>();
    let sender = actor.connection.network.sender.clone();

    let mut processing = actor.process();
    let mut snapshots = processing.stream().unwrap();
    processing.start().await.unwrap();

    // Initial is 0
    assert_eq!(snapshots.next().await.unwrap(), 0);

    // Both Ping and Pong should increment the counter
    sender.send(InMemoryEnvelope::wrap(Ping)).unwrap();
    assert_eq!(snapshots.next().await.unwrap(), 1);

    sender.send(InMemoryEnvelope::wrap(Pong)).unwrap();
    assert_eq!(snapshots.next().await.unwrap(), 2);

    processing.stop().await.unwrap();
  }

  #[tokio::test]
  async fn ping_pong_exchange_via_snapshots() {
    let runtime = Runtime::<InMemory>::new();

    let ping =
      runtime.spawn(PingPlayer { count: 0, max_count: 5 }).with_handler::<Pong>().with_name("ping");

    let pong = runtime.spawn(PongPlayer).with_handler::<Ping>().with_name("pong");

    let mut ping = ping.process();
    let mut snapshots = ping.stream().unwrap();
    ping.start().await.unwrap();

    let mut pong = pong.process();
    pong.start().await.unwrap();

    // PingPlayer starts at 0, increments each time it receives a Pong,
    // stops at max_count. We should see snapshots 0, 1, 2, 3, 4, 5
    // then the stream should end (actor stopped itself).
    for expected in 0..=5 {
      let snapshot = snapshots.next().await.unwrap();
      assert_eq!(snapshot, expected);
    }

    // Stream ends after self-stop
    assert_eq!(snapshots.next().await, None);
  }

  #[tokio::test]
  async fn actor_name_builder() {
    let runtime = Runtime::<InMemory>::new();
    let actor = runtime.spawn(Counter { count: 0 }).with_name("my-counter");
    assert_eq!(actor.name.as_deref(), Some("my-counter"));
  }

  #[tokio::test]
  async fn stream_taken_twice_errors() {
    let runtime = Runtime::<InMemory>::new();
    let actor = runtime.spawn(Counter { count: 0 });
    let mut processing = actor.process();

    let _stream = processing.stream().unwrap();
    let err = processing.stream().unwrap_err();
    assert!(matches!(err, crate::error::ArbiterError::StreamAlreadyTaken));
  }
}
