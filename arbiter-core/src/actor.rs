use std::{any::TypeId, collections::HashMap, fmt::Debug};

use crate::{
  handler::{
    Envelope, HandleResult, Handler, Message, MessageHandlerFn, Package, Unpackage, create_handler,
  },
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
    N::Payload: Unpackage<M> + Package<L::Reply>,
  {
    self.handlers.insert(TypeId::of::<M>(), create_handler::<M, L, N>());
    self
  }

  pub fn process(self) -> Processing<Self, L, N>
  where
    N::Payload: Package<L::StartMessage> + Package<L::StopMessage>,
  {
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
                connection.network.send(Envelope::package(start_message)).await;
              },
              Some(crate::processor::ControlSignal::Stop) => {
                state = State::Stopped;
                inner_controller.state_sender.send(State::Stopped).await.unwrap();
                let stop_message = inner.on_stop();
                connection.network.send(Envelope::package(stop_message)).await;
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
              && let Some(handler) = handlers.get(&message.type_id) {
                // Safe: `inner` and `handlers` are separate locals, no aliasing.
                let reply = handler(&mut inner as &mut dyn std::any::Any, message.payload);

                match reply {
                  HandleResult::Message(reply_envelope) => {
                    connection.network.send(reply_envelope).await;
                    let snapshot = inner.snapshot();
                    let _ = inner_controller.snapshot_sender.send(snapshot);
                  },
                  HandleResult::None => {
                    let snapshot = inner.snapshot();
                    let _ = inner_controller.snapshot_sender.send(snapshot);
                  },
                  HandleResult::Stop => {
                    state = State::Stopped;
                    inner_controller.state_sender.send(State::Stopped).await.unwrap();
                    let stop_message = inner.on_stop();
                    connection.network.send(Envelope::package(stop_message)).await;
                    break;
                  },
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
  use super::*;
  use crate::{fixtures::*, handler::Envelope, network::memory::InMemory, processor::State};

  #[tokio::test]
  async fn test_actor_lifecycle() {
    let network = InMemory::new();
    let actor = Actor::<Logger, InMemory>::new(Logger { message_count: 0 }, &network);
    assert_eq!(actor.state, State::Stopped);

    let mut processing = actor.process();
    processing.start().await.unwrap();
    assert_eq!(processing.state().await.unwrap(), State::Running);

    processing.stop().await.unwrap();
    let joined = processing.join().await.unwrap();
    assert_eq!(joined.state, State::Stopped);
  }

  #[tokio::test]
  async fn test_single_handler() {
    let network = InMemory::new();
    let actor = Actor::<Logger, InMemory>::new(Logger { message_count: 0 }, &network)
      .with_handler::<TextMessage>();

    let sender = actor.connection.network.sender.clone();

    let mut processing = actor.process();
    processing.start().await.unwrap();
    assert_eq!(processing.state().await.unwrap(), State::Running);

    sender.send(Envelope::package(TextMessage { content: "Hello".to_string() })).unwrap();

    tokio::time::sleep(std::time::Duration::from_millis(10)).await;

    processing.stop().await.unwrap();
    let actor = processing.join().await.unwrap();
    assert_eq!(actor.state, State::Stopped);
    assert_eq!(actor.inner.message_count, 1);
  }

  #[tokio::test]
  async fn test_multiple_handlers() {
    let network = InMemory::new();
    let actor = Actor::<Logger, InMemory>::new(Logger { message_count: 0 }, &network)
      .with_handler::<TextMessage>()
      .with_handler::<NumberMessage>();
    let sender = actor.connection.network.sender.clone();

    assert_eq!(actor.state, State::Stopped);

    let mut processing = actor.process();

    processing.start().await.unwrap();
    sender.send(Envelope::package(TextMessage { content: "Hello".to_string() })).unwrap();
    sender.send(Envelope::package(NumberMessage { value: 3 })).unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;

    processing.stop().await.unwrap();
    let actor = processing.join().await.unwrap();
    assert_eq!(actor.state, State::Stopped);
    assert_eq!(actor.inner.message_count, 2);
  }
}
