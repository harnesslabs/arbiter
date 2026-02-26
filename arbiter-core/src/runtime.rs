use std::{any::TypeId, collections::HashMap};

use crate::{
  agent::{Agent, LifeCycle},
  environment::Environment,
  handler::{Envelope, Handler, MessageHandlerFn, Package, Unpackage, create_handler},
  network::{Connection, Generateable, Network, memory::InMemory},
  processor::{CreateProcessor, State},
};

pub struct Runtime<N: Network, E: Environment = ()> {
  pub name: Option<String>,
  pub state: State,
  pub environment: E,
  pub connection: Connection<N>,
  pub handlers: HashMap<TypeId, MessageHandlerFn<N>>,
}

impl<N: Network, E: Environment> Runtime<N, E> {
  pub fn new() -> Self
  where
    <N as Network>::Payload: Package<<E as Environment>::Instruction>
      + Unpackage<<E as Environment>::Instruction>
      + Package<<E as Handler<<E as Environment>::Instruction>>::Reply>,
  {
    let mut handlers = HashMap::new();
    handlers.insert(TypeId::of::<E::Instruction>(), create_handler::<E::Instruction, E, N>());
    Self {
      name: None,
      state: State::Stopped,
      environment: E::new(),
      connection: Connection { address: N::Address::generate(), network: N::new().join() },
      handlers,
    }
  }

  pub fn spawn<L: LifeCycle>(&self, agent: L) -> Agent<L, N> {
    Agent::join(agent, &self.connection.network)
  }

  pub async fn broadcast_snapshot(&self)
  where
    N::Payload: Package<E::Snapshot>,
  {
    let snapshot = self.environment.snapshot();
    self.connection.network.send(Envelope::package(snapshot)).await;
  }
}

impl<E: Environment> CreateProcessor<E> for Runtime<InMemory, E> {
  fn name(&self) -> Option<String> {
    self.name.as_ref().map(String::from)
  }

  fn address(&self) -> <InMemory as Network>::Address {
    self.connection.address
  }

  fn connection(&mut self) -> &mut crate::network::Connection<InMemory> {
    &mut self.connection
  }

  fn handlers(
    &self,
  ) -> &std::collections::HashMap<std::any::TypeId, crate::handler::MessageHandlerFn<InMemory>> {
    &self.handlers
  }

  fn get_state(&self) -> State {
    self.state
  }

  fn set_state(&mut self, state: State) {
    self.state = state;
  }

  fn inner(&self) -> &E {
    &self.environment
  }

  fn inner_mut(&mut self) -> &mut E {
    &mut self.environment
  }

  fn into_inner(self) -> E {
    self.environment
  }
}
