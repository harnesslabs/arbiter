use std::{any::TypeId, collections::HashMap, fmt::Debug};

use tokio::task::JoinHandle;

use crate::{
  handler::{
    Envelope, HandleResult, Handler, HandlerError, Message, MessageHandlerFn, Package, Unpacackage,
    create_handler,
  },
  network::{Connection, Generateable, Network, memory::InMemory},
  protocol::{AgentId, MessageKind, SchemaVersion},
};

pub struct Agent<L: LifeCycle, N: Network> {
  pub name: Option<String>,
  state: State,
  inner: L,
  connection: Connection<N>,
  handlers: HashMap<TypeId, MessageHandlerFn<N>>,
  wire_handlers: HashMap<(MessageKind, SchemaVersion), TypeId>,
}

impl<L: LifeCycle, N: Network + Debug> Agent<L, N> {
  pub fn new(agent_inner: L) -> Self {
    let address = N::Address::generate();
    Self {
      name: None,
      state: State::Stopped,
      inner: agent_inner,
      connection: Connection::<N>::new(address),
      handlers: HashMap::new(),
      wire_handlers: HashMap::new(),
    }
  }

  pub fn new_join_network(agent_inner: L, network: &N) -> Self {
    Self {
      name: None,
      state: State::Stopped,
      inner: agent_inner,
      connection: Connection { address: N::Address::generate(), network: network.join() },
      handlers: HashMap::new(),
      wire_handlers: HashMap::new(),
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
    L: Handler<M>,
    N::Payload: Unpacackage<M> + Package<L::Reply>,
  {
    self.register_handler::<M>(MessageKind::for_type::<M>(), SchemaVersion::default());
    self
  }

  pub fn with_handler_kind<M>(
    mut self,
    message_kind: impl Into<MessageKind>,
    schema_version: SchemaVersion,
  ) -> Self
  where
    M: Message,
    L: Handler<M>,
    N::Payload: Unpacackage<M> + Package<L::Reply>,
  {
    self.register_handler::<M>(message_kind.into(), schema_version);
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

  fn register_handler<M>(&mut self, message_kind: MessageKind, schema_version: SchemaVersion)
  where
    M: Message,
    L: Handler<M>,
    N::Payload: Unpacackage<M> + Package<L::Reply>,
  {
    let message_type_id = TypeId::of::<M>();
    self.handlers.entry(message_type_id).or_insert_with(create_handler::<M, L, N>);
    self.wire_handlers.insert((message_kind, schema_version), message_type_id);
  }

  fn resolve_handler_type_id(&self, message: &Envelope<N>) -> Option<TypeId> {
    if self.handlers.contains_key(&message.type_id) {
      return Some(message.type_id);
    }

    self
      .wire_handlers
      .get(&(message.meta.message_kind.clone(), message.meta.schema_version))
      .copied()
  }
}

pub struct ProcessingAgent<L: LifeCycle, T: Network + Debug> {
  pub name: Option<String>,
  pub address: T::Address,
  pub(crate) task: JoinHandle<Agent<L, T>>,
  pub(crate) outer_controller: OuterController,
}

impl<L: LifeCycle, T: Network + Debug> ProcessingAgent<L, T> {
  pub fn name(&self) -> Option<&str> {
    self.name.as_deref()
  }

  pub const fn address(&self) -> T::Address {
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

  pub async fn join(self) -> Agent<L, T> {
    self.task.await.unwrap()
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
pub struct InnerController {
  pub(crate) instruction_receiver: tokio::sync::mpsc::Receiver<ControlSignal>,
  pub(crate) state_sender: tokio::sync::mpsc::Sender<State>,
}

pub struct OuterController {
  pub(crate) instruction_sender: tokio::sync::mpsc::Sender<ControlSignal>,
  pub(crate) state_receiver: tokio::sync::mpsc::Receiver<State>,
}

pub struct Controller {
  pub(crate) inner: InnerController,
  pub(crate) outer: OuterController,
}

impl Controller {
  // TODO: Add a default and let new take in pareameters for different channel implementations.
  #[allow(clippy::new_without_default)]
  pub fn new() -> Self {
    let (instruction_sender, instruction_receiver) = tokio::sync::mpsc::channel(8);
    let (state_sender, state_receiver) = tokio::sync::mpsc::channel(8);
    Self {
      inner: InnerController { instruction_receiver, state_sender },
      outer: OuterController { instruction_sender, state_receiver },
    }
  }
}

pub trait LifeCycle: Send + Sync + 'static {
  type StartMessage: Message + Debug;
  type StopMessage: Message + Debug;
  fn on_start(&mut self) -> Self::StartMessage;
  fn on_stop(&mut self) -> Self::StopMessage;
}

impl<L: LifeCycle> Agent<L, InMemory> {
  pub fn process(mut self) -> ProcessingAgent<L, InMemory> {
    let name = self.name.clone();
    let address = self.address();
    let local_agent_id = AgentId::from(address.to_string());
    let controller = Controller::new();
    let mut inner_controller = controller.inner;
    let outer_controller = controller.outer;

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
                self
                  .connection
                  .network
                  .send(Envelope::package(start_message).with_sender(local_agent_id.clone()))
                  .await;
              },
              Some(ControlSignal::Stop) => {
                self.state = State::Stopped;
                inner_controller.state_sender.send(State::Stopped).await.unwrap();
                let stop_message = self.inner.on_stop();
                self
                  .connection
                  .network
                  .send(Envelope::package(stop_message).with_sender(local_agent_id.clone()))
                  .await;
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
              if !message.meta.recipient.matches_agent(&local_agent_id) {
                continue;
              }

              println!("received message {:?} for agent {}", message, self.name.as_deref().unwrap_or("unknown"));
              if let Some(message_type_id) = self.resolve_handler_type_id(&message) {
                let handler = self.handlers.get(&message_type_id).expect("handler id registry drift");
                let reply = handler(&mut self.inner, message);
                println!("reply for agent {}", self.name.as_deref().unwrap_or("unknown"));
                match reply {
                  Ok(HandleResult::Message(message)) => {
                    let message = message.with_sender(local_agent_id.clone());
                    println!("sending reply {:?} for agent {}", message, self.name.as_deref().unwrap_or("unknown"));
                    self.connection.network.send(message).await;
                  },
                  Ok(HandleResult::None) => {},
                  Ok(HandleResult::Stop) => break,
                  Err(error) => {
                    log_handler_error(self.name.as_deref().unwrap_or("unknown"), &error);
                  },
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

fn log_handler_error(agent_name: &str, error: &HandlerError) {
  eprintln!("handler error for agent {agent_name}: {error}");
}

#[cfg(test)]
mod tests {

  use super::*;
  use crate::{
    fixtures::*,
    protocol::{EnvelopeMeta, SchemaVersion},
  };

  #[tokio::test]
  async fn test_agent_lifecycle() {
    let agent =
      Agent::<Logger, InMemory>::new(Logger { name: "TestLogger".to_string(), message_count: 0 });
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
    let agent =
      Agent::<Logger, InMemory>::new(Logger { name: "TestLogger".to_string(), message_count: 0 })
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
    let mut agent =
      Agent::<Logger, InMemory>::new(Logger { name: "TestLogger".to_string(), message_count: 0 });
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

  #[tokio::test]
  async fn test_addressed_delivery_only_hits_target_agent() {
    let network = InMemory::new();
    let sender = network.sender.clone();

    let agent_a = Agent::<Logger, InMemory>::new_join_network(
      Logger { name: "a".to_string(), message_count: 0 },
      &network,
    )
    .with_handler::<TextMessage>();

    let target_address = agent_a.address();

    let agent_b = Agent::<Logger, InMemory>::new_join_network(
      Logger { name: "b".to_string(), message_count: 0 },
      &network,
    )
    .with_handler::<TextMessage>();

    let mut processing_a = agent_a.process();
    let mut processing_b = agent_b.process();

    processing_a.start().await;
    processing_b.start().await;

    sender
      .send(
        Envelope::package(TextMessage { content: "only-a".to_string() }).to_address(target_address),
      )
      .unwrap();

    tokio::time::sleep(std::time::Duration::from_millis(10)).await;

    processing_a.stop().await;
    processing_b.stop().await;

    let agent_a = processing_a.join().await;
    let agent_b = processing_b.join().await;

    assert_eq!(agent_a.inner.message_count, 1);
    assert_eq!(agent_b.inner.message_count, 0);
  }

  #[tokio::test]
  async fn test_wire_metadata_dispatch_falls_back_when_type_id_does_not_match() {
    let agent =
      Agent::<Logger, InMemory>::new(Logger { name: "fallback".to_string(), message_count: 0 })
        .with_handler::<TextMessage>();
    let sender = agent.connection.network.sender.clone();

    let mut processing_agent = agent.process();
    processing_agent.start().await;

    let mut envelope = Envelope::package(TextMessage { content: "hello".to_string() });
    envelope.type_id = std::any::TypeId::of::<NumberMessage>();
    sender.send(envelope).unwrap();

    tokio::time::sleep(std::time::Duration::from_millis(10)).await;

    processing_agent.stop().await;
    let agent = processing_agent.join().await;
    assert_eq!(agent.inner.message_count, 1);
  }

  #[tokio::test]
  async fn test_custom_message_kind_registration_dispatches_without_default_type_id_match() {
    let agent =
      Agent::<Logger, InMemory>::new(Logger { name: "custom-kind".to_string(), message_count: 0 })
        .with_handler_kind::<TextMessage>("app.text.event", SchemaVersion(7));
    let sender = agent.connection.network.sender.clone();

    let mut processing_agent = agent.process();
    processing_agent.start().await;

    let meta = EnvelopeMeta::new("app.text.event").with_schema_version(SchemaVersion(7));
    let mut envelope =
      Envelope::package_with_meta(TextMessage { content: "hello".to_string() }, meta);
    envelope.type_id = std::any::TypeId::of::<NumberMessage>();
    sender.send(envelope).unwrap();

    tokio::time::sleep(std::time::Duration::from_millis(10)).await;

    processing_agent.stop().await;
    let agent = processing_agent.join().await;
    assert_eq!(agent.inner.message_count, 1);
  }
}
