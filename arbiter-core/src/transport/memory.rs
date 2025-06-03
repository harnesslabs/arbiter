use std::{any::Any, collections::VecDeque, rc::Rc};

use super::Transport;
use crate::{agent::AgentIdentity, handler::Message, transport::SyncRuntime};

/// In-memory transport that preserves current Runtime behavior
pub struct InMemoryTransport {
  local_identity: AgentIdentity,
  message_queue:  VecDeque<Rc<dyn Any>>,
}

// TODO: This is a hack to get around the fact that we need to store messages in a VecDeque
// We need to find a better way to do this.
// impl From<Box<dyn Message>> for Rc<dyn Any> {
//   fn from(message: Box<dyn Message>) -> Self { Rc::new(Box::leak(message)) }
// }

impl Transport<SyncRuntime> for InMemoryTransport {
  type Address = AgentIdentity;
  type Error = String;
  type Payload = Rc<dyn Any>;

  fn new() -> Self {
    Self { local_identity: AgentIdentity::generate(), message_queue: VecDeque::new() }
  }

  // For in-memory transport, "sending" just enqueues locally
  fn send(&mut self, payload: Self::Payload) -> Result<(), Self::Error> {
    self.message_queue.push_back(payload);
    Ok(())
  }

  fn poll(&mut self) -> Vec<Self::Payload> {
    let mut messages = Vec::new();
    while let Some(payload) = self.message_queue.pop_front() {
      messages.push(payload);
    }
    messages
  }

  fn local_address(&self) -> Self::Address { self.local_identity }
}

impl Default for InMemoryTransport {
  fn default() -> Self { Self::new() }
}
