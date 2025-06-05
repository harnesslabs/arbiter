use std::sync::Arc;

use crate::{
  agent::AgentIdentity,
  connection::{Connection, Receiver, Sender},
  handler::{Envelope, Message},
};

#[derive(Clone)]
pub struct InMemory {
  pub(crate) address: AgentIdentity,
  pub(crate) sender:  flume::Sender<Envelope<Self>>,
  receiver:           flume::Receiver<Envelope<Self>>,
}

impl Sender for flume::Sender<Envelope<InMemory>> {
  type Connection = InMemory;

  fn send(&self, envelope: Envelope<Self::Connection>) { self.send(envelope).unwrap(); }
}

impl Receiver for flume::Receiver<Envelope<InMemory>> {
  type Connection = InMemory;

  fn receive(&self) -> Option<Envelope<Self::Connection>> { self.try_recv().ok() }
}

impl Connection for InMemory {
  type Address = AgentIdentity;
  type Payload = Arc<dyn Message>;
  type Receiver = flume::Receiver<Envelope<Self>>;
  type Sender = flume::Sender<Envelope<Self>>;

  fn new() -> Self {
    let (sender, receiver) = flume::unbounded();
    Self { address: AgentIdentity::generate(), sender, receiver }
  }

  fn address(&self) -> Self::Address { self.address }

  fn sender(&mut self) -> &mut Self::Sender { &mut self.sender }

  fn receiver(&mut self) -> &mut Self::Receiver { &mut self.receiver }

  fn create_outbound_connection(&self) -> (Self::Address, Self::Sender) {
    (self.address, self.sender.clone())
  }
}
