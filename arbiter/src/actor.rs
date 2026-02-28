//! Actor and lifecycle abstractions for the Arbiter framework.
//!
//! This module defines the [`LifeCycle`] trait for defining actor behavior and
//! the [`Actor`] struct for managing actor state and message routing.

use std::{any::TypeId, collections::HashMap, fmt::Debug};

use crate::{
  handler::{Envelope, Handler, Message, MessageHandlerFn, create_handler},
  network::{Network, Socket},
  processor::{Controller, Processing, State},
};

/// Defines the behavior and state transitions of an actor.
pub trait LifeCycle: Send + Sync + 'static {
  /// The message type sent when the actor starts.
  type StartMessage: Message + Debug;
  /// The message type sent when the actor stops.
  type StopMessage: Message + Debug;
  /// A snapshot of the actor's state, useful for testing and observation.
  type Snapshot: Send + Sync + Clone + Debug + 'static;

  /// Hook called when the actor is started.
  fn on_start(&mut self) -> Self::StartMessage;

  /// Hook called when the actor is stopped.
  fn on_stop(&mut self) -> Self::StopMessage;

  /// Returns a snapshot of the current state.
  fn snapshot(&self) -> Self::Snapshot;

  /// Returns `true` if the actor should self-terminate.
  fn should_stop(&self) -> bool { false }
}

/// A fully configured actor, ready to be spawned into the runtime.
///
/// It encapsulates the user-defined `LifeCycle`, its addressable `Socket`,
/// and a dynamic mapping of message `TypeId`s to their respective handlers.
pub struct Actor<L: LifeCycle, N: Network> {
  pub name:            Option<String>,
  pub(crate) state:    State,
  pub(crate) inner:    L,
  pub(crate) socket:   N::Socket,
  pub(crate) handlers: HashMap<TypeId, MessageHandlerFn<N>>,
}

impl<L: LifeCycle, N: Network> Actor<L, N> {
  pub(crate) fn new(inner: L, socket: N::Socket) -> Self {
    Self { name: None, state: State::Stopped, inner, socket, handlers: HashMap::new() }
  }

  /// Sets the actor's name for debugging purposes.
  #[must_use]
  pub fn with_name(mut self, name: impl Into<String>) -> Self {
    self.name = Some(name.into());
    self
  }

  /// Mutates the actor's name in place.
  pub fn set_name(&mut self, name: impl Into<String>) { self.name = Some(name.into()); }

  /// Clears the actor's name.
  pub fn clear_name(&mut self) { self.name = None; }

  /// Registers a handler for the specified message type `M`.
  #[must_use]
  pub fn with_handler<M>(mut self) -> Self
  where
    M: Message,
    L: Handler<M>, {
    self.handlers.insert(TypeId::of::<M>(), create_handler::<M, L, N>());
    self
  }

  /// Transforms the actor into a running `Processing` task.
  /// This will consume the `Actor` configuration and spawn it onto a Tokio task.
  pub(crate) fn into_processing(self) -> Processing<Self, L, N> {
    let processing_name = self.name.clone();
    let address = self.socket.address();

    let controller = Controller::new();
    let mut inner_controller = controller.inner;
    let outer_controller = controller.outer;

    // Send the initial snapshot to observers
    let snapshot = self.inner.snapshot();
    inner_controller.snapshot_sender.send(snapshot).unwrap();

    let Self { name, mut state, mut inner, mut socket, handlers } = self;

    #[cfg(all(feature = "wasm", target_arch = "wasm32"))]
    let spawn = tokio_with_wasm::spawn;
    #[cfg(not(all(feature = "wasm", target_arch = "wasm32")))]
    let spawn = tokio::spawn;

    let task = spawn(async move {
      loop {
        let prev_state = state;
        tokio::select! {
          biased;

          control_signal = inner_controller.instruction_receiver.recv() => {
            match control_signal {
              Some(crate::processor::ControlSignal::Start) => {
                state = State::Running;
                let _ = inner_controller.state_sender.send(State::Running).await;
                let start_message = inner.on_start();
                tracing::debug!(agent = ?name, "sending start_message");
                socket.send(<<N as Network>::Socket as Socket>::Envelope::wrap(start_message)).await;
              },
              Some(crate::processor::ControlSignal::Pause) => {
                state = State::Paused;
                let _ = inner_controller.state_sender.send(State::Paused).await;
              },
              Some(crate::processor::ControlSignal::Stop) => {
                state = State::Stopped;
                let _ = inner_controller.state_sender.send(State::Stopped).await;
                let stop_message = inner.on_stop();
                tracing::debug!(agent = ?name, "sending stop_message");
                socket.send(<<N as Network>::Socket as Socket>::Envelope::wrap(stop_message)).await;
                break;
              },
              Some(crate::processor::ControlSignal::GetState) => {
                let _ = inner_controller.state_sender.send(prev_state).await;
              },
              None => {
                break;
              },
            }
          }

          message = socket.receive(), if state == State::Running => {
            if let Some(message) = message
              && let Some(handler) = handlers.get(&message.type_id()) {
                let reply = handler(&mut inner as &mut dyn std::any::Any, &message);

                if let Some(envelope) = reply {
                  socket.send(envelope).await;
                }

                let _ = inner_controller.snapshot_sender.send(inner.snapshot());

                if inner.should_stop() {
                  state = State::Stopped;
                  let _ = inner_controller.state_sender.send(State::Stopped).await;
                  let stop_message = inner.on_stop();
                  socket.send(<<N as Network>::Socket as Socket>::Envelope::wrap(stop_message)).await;
                  break;
                }
              }
          }
        }
      }

      Actor { name, state, inner, socket, handlers }
    });

    Processing { name: processing_name, address, task, outer_controller }
  }
}

#[cfg(test)]
mod tests {
  use tokio_stream::StreamExt;

  use crate::{
    fixtures::*,
    handler::Envelope as _,
    network::memory::{InMemory, InMemoryEnvelope},
    processor::State,
    runtime::Runtime,
  };

  #[tokio::test]
  async fn lifecycle_start_stop_join() {
    let mut runtime = Runtime::<InMemory>::new();
    let actor = runtime.spawn(Counter { count: 0 });

    let mut processing = runtime.process(actor);
    let mut snapshots = processing.stream().unwrap();

    processing.start().await.unwrap();
    assert_eq!(processing.state().await.unwrap(), State::Running);

    // Initial snapshot emitted at process() time
    assert_eq!(snapshots.next().await.unwrap(), 0);

    // stop() succeeds — actor exited cleanly and is returned
    let _actor = processing.stop().await.unwrap();

    // Stream closes after the actor exits
    assert_eq!(snapshots.next().await, None);
  }

  #[tokio::test]
  async fn single_handler_increments_snapshot() {
    let mut runtime = Runtime::<InMemory>::new();
    let actor = runtime.spawn(Counter { count: 0 }).with_handler::<Ping>();

    let mut processing = runtime.process(actor);
    let mut snapshots = processing.stream().unwrap();
    processing.start().await.unwrap();

    // Initial snapshot is 0
    assert_eq!(snapshots.next().await.unwrap(), 0);

    // Send a Ping, snapshot should become 1
    runtime.network().send(InMemoryEnvelope::wrap(Ping));
    assert_eq!(snapshots.next().await.unwrap(), 1);

    processing.stop().await.unwrap();
  }

  #[tokio::test]
  async fn multiple_handlers_route_correctly() {
    let mut runtime = Runtime::<InMemory>::new();
    let actor = runtime.spawn(Counter { count: 0 }).with_handler::<Ping>().with_handler::<Pong>();

    let mut processing = runtime.process(actor);
    let mut snapshots = processing.stream().unwrap();
    processing.start().await.unwrap();

    // Initial is 0
    assert_eq!(snapshots.next().await.unwrap(), 0);

    // Both Ping and Pong should increment the counter
    runtime.network().send(InMemoryEnvelope::wrap(Ping));
    assert_eq!(snapshots.next().await.unwrap(), 1);

    runtime.network().send(InMemoryEnvelope::wrap(Pong));
    assert_eq!(snapshots.next().await.unwrap(), 2);

    processing.stop().await.unwrap();
  }

  #[tokio::test]
  async fn ping_pong_exchange_via_snapshots() {
    let mut runtime = Runtime::<InMemory>::new();

    let ping =
      runtime.spawn(PingPlayer { count: 0, max_count: 5 }).with_handler::<Pong>().with_name("ping");

    let pong = runtime.spawn(PongPlayer).with_handler::<Ping>().with_name("pong");

    let mut ping = runtime.process(ping);
    let mut pong = runtime.process(pong);

    let mut snapshots = ping.stream().unwrap();

    ping.start().await.unwrap();
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
    let mut runtime = Runtime::<InMemory>::new();
    let actor = runtime.spawn(Counter { count: 0 }).with_name("my-counter");
    assert_eq!(actor.name.as_deref(), Some("my-counter"));
  }

  #[tokio::test]
  async fn stream_taken_twice_errors() {
    let mut runtime = Runtime::<InMemory>::new();
    let actor = runtime.spawn(Counter { count: 0 });
    let mut processing = runtime.process(actor);

    let _stream = processing.stream().unwrap();
    let err = processing.stream().unwrap_err();
    assert!(matches!(err, crate::error::ArbiterError::StreamAlreadyTaken));
  }
}
