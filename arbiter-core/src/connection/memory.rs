use std::sync::Arc;

use super::Transport;
use crate::{
  agent::{AgentIdentity, Controller},
  connection::{Connection, SyncRuntime},
  handler::{Envelope, Message},
};

#[derive(Clone)]
pub struct InMemoryConnection {
  pub(crate) address: AgentIdentity,
  sender:             flume::Sender<Envelope<Self>>,
  receiver:           flume::Receiver<Envelope<Self>>,
}

impl Connection for InMemoryConnection {
  type Address = AgentIdentity;
  type Payload = Arc<dyn Message>;
  type Receiver = flume::Receiver<Envelope<Self>>;
  type Sender = flume::Sender<Envelope<Self>>;

  fn new() -> Self {
    let (sender, receiver) = flume::unbounded();
    Self { address: AgentIdentity::generate(), sender, receiver }
  }

  fn address(&self) -> Self::Address { self.address }

  fn get_sender(&self) -> Self::Sender { self.sender.clone() }

  fn send(&mut self, envelope: Envelope<Self>) { self.sender.send(envelope).unwrap(); }

  fn receive(&mut self) -> Option<Envelope<Self>> { self.receiver.try_recv().ok() }
}
