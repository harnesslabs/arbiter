use std::{any::TypeId, collections::HashMap, fmt::Debug, sync::Arc};

use tokio::{sync::Mutex, task::JoinHandle};
use tokio_stream::wrappers::UnboundedReceiverStream;

use crate::{
  environment::Environment,
  handler::{
    Envelope, HandleResult, Handler, Message, MessageHandlerFn, Package, Unpacackage,
    create_handler,
  },
  network::{Connection, Generateable, Network, memory::InMemory},
};

// TODO: Observing snapshots should be an optional gate. We don't have to always snapshot.

pub struct Agent<L: LifeCycle, N: Network, E: Environment = ()> {
  pub name: Option<String>,
  state: State,
  inner: L,
  connection: Connection<N>,
  environment: Arc<Mutex<E>>,
  handlers: HashMap<TypeId, MessageHandlerFn<N, E>>,
}

impl<L: LifeCycle, N: Network, E: Environment> Agent<L, N, E> {
  pub(crate) fn join(agent_inner: L, network: &N, environment: Arc<Mutex<E>>) -> Self {
    Self {
      name: None,
      state: State::Stopped,
      inner: agent_inner,
      connection: Connection { address: N::Address::generate(), network: network.join() },
      environment,
      handlers: HashMap::new(),
    }
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
    L: Handler<M, E>,
    N::Payload: Unpacackage<M> + Package<L::Reply>,
  {
    self.handlers.insert(TypeId::of::<M>(), create_handler::<M, L, N, E>());
    self
  }

  pub const fn address(&self) -> N::Address {
    self.connection.address
  }

  pub fn name(&self) -> Option<&str> {
    self.name.as_deref()
  }

  pub const fn network(&self) -> &N {
    &self.connection.network
  }

  pub const fn inner(&self) -> &L {
    &self.inner
  }

  pub const fn inner_mut(&mut self) -> &mut L {
    &mut self.inner
  }

  pub const fn state(&self) -> State {
    self.state
  }
}

pub struct ProcessingAgent<L: LifeCycle, N: Network + Debug, E: Environment> {
  pub name: Option<String>,
  pub address: N::Address,
  pub(crate) task: JoinHandle<Agent<L, N, E>>,
  pub(crate) outer_controller: OuterController<L>,
}

// TODO: Handle errors properly in here as it's possible to send instructions with the channel down.
impl<L: LifeCycle, N: Network + Debug, E: Environment> ProcessingAgent<L, N, E> {
  pub fn name(&self) -> Option<&str> {
    self.name.as_deref()
  }

  pub const fn address(&self) -> N::Address {
    self.address
  }

  pub async fn state(&mut self) -> State {
    self.outer_controller.instruction_sender.send(ControlSignal::GetState).await.unwrap();
    self.outer_controller.state_receiver.recv().await.unwrap()
  }

  pub async fn start(&mut self) {
    self.outer_controller.instruction_sender.send(ControlSignal::Start).await.unwrap();
    let state = self.outer_controller.state_receiver.recv().await.unwrap();
    assert_eq!(state, State::Running);
  }

  pub async fn stop(&mut self) {
    self.outer_controller.instruction_sender.send(ControlSignal::Stop).await.unwrap();
    let state = self.outer_controller.state_receiver.recv().await.unwrap();
    assert_eq!(state, State::Stopped);
  }

  pub async fn join(self) -> Agent<L, N, E> {
    self.task.await.unwrap()
  }

  // TODO: Calling stream twice would break things, so this needs fixed.
  pub async fn stream(&mut self) -> UnboundedReceiverStream<L::Snapshot> {
    let recv = self.outer_controller.snapshot_receiver.take().unwrap();
    tokio_stream::wrappers::UnboundedReceiverStream::new(recv)
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
  Stopped,
  Running,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlSignal {
  Start,
  Stop,
  GetState,
}

// TODO (autoparallel): These controllers are hard-coded to use flume, we should use a more generic
// controller that can be used with any channel implementation.
pub struct InnerController<L: LifeCycle> {
  pub(crate) instruction_receiver: tokio::sync::mpsc::Receiver<ControlSignal>,
  pub(crate) state_sender: tokio::sync::mpsc::Sender<State>,
  pub(crate) snapshot_sender: tokio::sync::mpsc::UnboundedSender<L::Snapshot>,
}

pub struct OuterController<L: LifeCycle> {
  pub(crate) instruction_sender: tokio::sync::mpsc::Sender<ControlSignal>,
  pub(crate) state_receiver: tokio::sync::mpsc::Receiver<State>,
  pub(crate) snapshot_receiver: Option<tokio::sync::mpsc::UnboundedReceiver<L::Snapshot>>,
}

pub struct Controller<L: LifeCycle> {
  pub(crate) inner: InnerController<L>,
  pub(crate) outer: OuterController<L>,
}

impl<L: LifeCycle> Controller<L> {
  // TODO: Add a default and let new take in pareameters for different channel implementations.
  #[allow(clippy::new_without_default)]
  pub fn new() -> Self {
    let (instruction_sender, instruction_receiver) = tokio::sync::mpsc::channel(8);
    let (state_sender, state_receiver) = tokio::sync::mpsc::channel(8);
    let (snapshot_sender, snapshot_receiver) = tokio::sync::mpsc::unbounded_channel();
    Self {
      inner: InnerController { instruction_receiver, state_sender, snapshot_sender },
      outer: OuterController {
        instruction_sender,
        state_receiver,
        snapshot_receiver: Some(snapshot_receiver),
      },
    }
  }
}

pub trait LifeCycle: Send + Sync + 'static {
  type StartMessage: Message + Debug;
  type StopMessage: Message + Debug;
  type Snapshot: Send + Sync + Clone + Debug + 'static;
  fn on_start(&mut self) -> Self::StartMessage;
  fn on_stop(&mut self) -> Self::StopMessage;
  fn snapshot(&self) -> Self::Snapshot;
}

impl<L: LifeCycle, E: Environment> Agent<L, InMemory, E> {
  pub fn process(mut self) -> ProcessingAgent<L, InMemory, E> {
    let name = self.name.clone();
    let address = self.address();
    let controller = Controller::new();
    let mut inner_controller = controller.inner;
    let outer_controller = controller.outer;

    // ────────────────────────────────────────────────────────────────
    // Update Observers with snapshots of the current state
    // ────────────────────────────────────────────────────────────────
    let snapshot = self.inner.snapshot();
    inner_controller.snapshot_sender.send(snapshot).unwrap();

    let task = tokio::spawn(async move {
      loop {
        // ────────────────────────────────────────────────────────────────
        // Control-plane messages (START / STOP / GET_STATE)
        // ────────────────────────────────────────────────────────────────
        let prev_state = self.state;
        tokio::select! {
          biased;
          control_signal = inner_controller.instruction_receiver.recv() => {
            match control_signal {
              Some(ControlSignal::Start) => {
                self.state = State::Running;
                inner_controller.state_sender.send(State::Running).await.unwrap();
                let start_message = self.inner.on_start();
                println!("sending start_message for agent {}", self.name.as_deref().unwrap_or("unknown"));
                self.connection.network.send(Envelope::package(start_message)).await;
              },
              Some(ControlSignal::Stop) => {
                self.state = State::Stopped;
                inner_controller.state_sender.send(State::Stopped).await.unwrap();
                let stop_message = self.inner.on_stop();
                self.connection.network.send(Envelope::package(stop_message)).await;
                break;
              },
              Some(ControlSignal::GetState) => {
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
          message = self.connection.network.receive() => {
            if let Some(message) = message {
              println!("received message {:?} for agent {}", message, self.name.as_deref().unwrap_or("unknown"));
              if let Some(handler) = self.handlers.get(&message.type_id) {
                let reply = handler(&mut self.inner, message.payload);
                println!("reply for agent {}", self.name.as_deref().unwrap_or("unknown"));
                // ────────────────────────────────────────────────────────────────
                // Handle the reply from the message handler and send it back over the network if needed
                // ────────────────────────────────────────────────────────────────
                match reply {
                  HandleResult::Message(message) => {
                    println!("sending reply {:?} for agent {}", message, self.name.as_deref().unwrap_or("unknown"));
                    self.connection.network.send(message).await;

                    // Update Observers with snapshots of the current state
                    let snapshot = self.inner.snapshot();
                    inner_controller.snapshot_sender.send(snapshot).unwrap();
                  },
                  HandleResult::Update(update) => {
                    // Update the environment with the update from the handler
                    let mut environment = self.environment.lock().await;
                    environment.update_state(update);

                    // Update Observers with snapshots of the current state
                    let snapshot = self.inner.snapshot();
                    inner_controller.snapshot_sender.send(snapshot).unwrap();
                  },
                  HandleResult::None => {
                    // Update Observers with snapshots of the current state
                    let snapshot = self.inner.snapshot();
                    inner_controller.snapshot_sender.send(snapshot).unwrap();
                  },
                  HandleResult::Stop => break,
                }
              }
            }
          }

        }
      }

      self
    });

    ProcessingAgent { name, address, task, outer_controller }
  }
}

#[cfg(test)]
mod tests {

  use super::*;
  use crate::fixtures::*;

  #[tokio::test]
  async fn test_agent_lifecycle() {
    let network = InMemory::new();
    let agent = Agent::<Logger, InMemory>::join(
      Logger { message_count: 0 },
      &network,
      Arc::new(Mutex::new(())),
    );
    assert_eq!(agent.state, State::Stopped);

    let mut processing_agent = agent.process();
    processing_agent.start().await;
    assert_eq!(processing_agent.state().await, State::Running);

    processing_agent.stop().await;
    let joined_agent = processing_agent.join().await;
    assert_eq!(joined_agent.state, State::Stopped);
  }

  #[tokio::test]
  async fn test_single_agent_handler() {
    let network = InMemory::new();
    let agent = Agent::<Logger, InMemory>::join(
      Logger { message_count: 0 },
      &network,
      Arc::new(Mutex::new(())),
    )
    .with_handler::<TextMessage>();

    // Grab a sender from the agent
    let sender = agent.connection.network.sender.clone();

    let mut processing_agent = agent.process();
    processing_agent.start().await;
    assert_eq!(processing_agent.state().await, State::Running);

    // Send a message to the agent
    sender.send(Envelope::package(TextMessage { content: "Hello".to_string() })).unwrap();

    tokio::time::sleep(std::time::Duration::from_millis(10)).await;

    processing_agent.stop().await;
    let agent = processing_agent.join().await;
    assert_eq!(agent.state, State::Stopped);
    assert_eq!(agent.inner.message_count, 1);
  }

  #[tokio::test]
  async fn test_multiple_agent_handlers() {
    let network = InMemory::new();
    let mut agent = Agent::<Logger, InMemory>::join(
      Logger { message_count: 0 },
      &network,
      Arc::new(Mutex::new(())),
    );
    agent = agent.with_handler::<TextMessage>().with_handler::<NumberMessage>();
    let sender = agent.connection.network.sender.clone();

    assert_eq!(agent.state, State::Stopped);

    let mut processing_agent = agent.process();

    processing_agent.start().await;
    sender.send(Envelope::package(TextMessage { content: "Hello".to_string() })).unwrap();
    sender.send(Envelope::package(NumberMessage { value: 3 })).unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;

    processing_agent.stop().await;
    let agent = processing_agent.join().await;
    assert_eq!(agent.state, State::Stopped);
    assert_eq!(agent.inner.message_count, 2);
  }
}
