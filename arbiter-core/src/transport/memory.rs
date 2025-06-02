use std::{
  any::{Any, TypeId},
  collections::VecDeque,
  rc::Rc,
};

use super::{AgentIdentity, Envelope, TransportLayer};

/// In-memory transport that preserves current Runtime behavior
pub struct InMemoryTransport {
  local_identity: AgentIdentity,
  message_queue:  VecDeque<Envelope<Self>>,
}

impl InMemoryTransport {
  pub fn new() -> Self {
    Self { local_identity: AgentIdentity::generate(), message_queue: VecDeque::new() }
  }
}

impl TransportLayer for InMemoryTransport {
  type Address = AgentIdentity;
  type Error = String;
  type Payload = Rc<dyn Any>;

  // For in-memory transport, "sending" just enqueues locally
  fn send(&mut self, envelope: Envelope<Self>) -> Result<(), Self::Error> {
    self.message_queue.push_back(envelope);
    Ok(())
  }

  fn poll(&mut self) -> Vec<Envelope<Self>> {
    let mut messages = Vec::new();
    while let Some(envelope) = self.message_queue.pop_front() {
      messages.push(envelope);
    }
    messages
  }

  fn local_address(&self) -> Self::Address { self.local_identity }
}

/// In-memory envelope that preserves current Rc<dyn Any> semantics
#[derive(Clone)]
pub struct InMemoryEnvelope {
  from:         AgentIdentity,
  to:           AgentIdentity,
  message:      Rc<dyn Any>,
  message_type: TypeId,
}

impl InMemoryEnvelope {
  pub fn new_with_message<M: Any + 'static>(
    from: AgentIdentity,
    to: AgentIdentity,
    message: M,
  ) -> Self {
    Self { from, to, message: Rc::new(message), message_type: TypeId::of::<M>() }
  }

  pub fn message(&self) -> &Rc<dyn Any> { &self.message }

  pub fn message_type(&self) -> TypeId { self.message_type }
}
