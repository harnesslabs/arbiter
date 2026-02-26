use std::{any::TypeId, collections::HashMap, fmt::Debug};

use crate::{
  handler::{Handler, Message, MessageHandlerFn, Package, Unpackage, create_handler},
  network::{Connection, Generateable, Network, memory::InMemory},
  processor::{CreateProcessor, State},
};

// TODO: Observing snapshots should be an optional gate. We don't have to always snapshot.

pub struct Agent<L: LifeCycle, N: Network> {
  pub name: Option<String>,
  state: State,
  inner: L,
  connection: Connection<N>,
  handlers: HashMap<TypeId, MessageHandlerFn<N>>,
}

impl<L: LifeCycle, N: Network> Agent<L, N> {
  pub(crate) fn join(agent_inner: L, network: &N) -> Self {
    Self {
      name: None,
      state: State::Stopped,
      inner: agent_inner,
      connection: Connection { address: N::Address::generate(), network: network.join() },
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
    L: Handler<M>,
    N::Payload: Unpackage<M> + Package<L::Reply>,
  {
    self.handlers.insert(TypeId::of::<M>(), create_handler::<M, L, N>());
    self
  }
}

impl<L: LifeCycle> CreateProcessor<L> for Agent<L, InMemory> {
  fn name(&self) -> Option<String> {
    self.name.as_ref().map(String::from)
  }

  fn address(&self) -> <InMemory as Network>::Address {
    self.connection.address
  }

  fn connection(&mut self) -> &mut Connection<crate::network::memory::InMemory> {
    &mut self.connection
  }

  fn handlers(&self) -> &HashMap<TypeId, MessageHandlerFn<crate::network::memory::InMemory>> {
    &self.handlers
  }

  fn inner(&self) -> &L {
    &self.inner
  }

  fn inner_mut(&mut self) -> &mut L {
    &mut self.inner
  }

  fn into_inner(self) -> L {
    self.inner
  }

  fn get_state(&self) -> State {
    self.state
  }

  fn set_state(&mut self, state: State) {
    self.state = state;
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

#[cfg(test)]
mod tests {

  use super::*;
  use crate::{fixtures::*, handler::Envelope, network::memory::InMemory, processor::State};

  #[tokio::test]
  async fn test_agent_lifecycle() {
    let network = InMemory::new();
    let agent = Agent::<Logger, InMemory>::join(Logger { message_count: 0 }, &network);
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
    let agent = Agent::<Logger, InMemory>::join(Logger { message_count: 0 }, &network)
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
    let mut agent = Agent::<Logger, InMemory>::join(Logger { message_count: 0 }, &network);
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
