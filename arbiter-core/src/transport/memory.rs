use std::{any::Any, collections::VecDeque, future::Future, pin::Pin, rc::Rc};

use super::{Envelope, Transport};
use crate::{
  agent::AgentIdentity,
  handler::Message,
  transport::{AsyncRuntime, Runtime, SyncRuntime},
};

/// In-memory transport that preserves current Runtime behavior
pub struct InMemoryTransport {
  local_identity: AgentIdentity,
  message_queue:  VecDeque<Envelope<Self, SyncRuntime>>,
}

// TODO: This is a hack to get around the fact that we need to store messages in a VecDeque
// We need to find a better way to do this.
impl From<Box<dyn Message>> for Rc<dyn Any> {
  fn from(message: Box<dyn Message>) -> Self { Rc::new(Box::leak(message)) }
}

impl Transport<SyncRuntime> for InMemoryTransport {
  type Address = AgentIdentity;
  type Error = String;
  type Payload = Rc<dyn Any>;

  fn new() -> Self {
    Self { local_identity: AgentIdentity::generate(), message_queue: VecDeque::new() }
  }

  // For in-memory transport, "sending" just enqueues locally
  fn send(&mut self, envelope: Envelope<Self, SyncRuntime>) -> Result<(), Self::Error> {
    self.message_queue.push_back(envelope);
    Ok(())
  }

  fn poll(&mut self) -> Vec<Envelope<Self, SyncRuntime>> {
    let mut messages = Vec::new();
    while let Some(envelope) = self.message_queue.pop_front() {
      messages.push(envelope);
    }
    messages
  }

  fn local_address(&self) -> Self::Address { self.local_identity }
}

impl Default for InMemoryTransport {
  fn default() -> Self { Self::new() }
}
