use std::sync::Arc;

use super::Transport;
use crate::{
  agent::AgentIdentity,
  handler::{Envelope, Message},
  transport::SyncRuntime,
};

// TODO: Perhaps Transport really should be a struct.
/// In-memory transport that preserves current Runtime behavior
pub struct InMemoryTransport {
  local_identity: AgentIdentity,
  connections:    Vec<flume::Sender<Envelope<Self>>>,
}

impl Transport for InMemoryTransport {
  type Address = AgentIdentity;
  type Error = String;
  type Payload = Arc<dyn Message>;
  type Runtime = SyncRuntime;

  fn new() -> Self { Self { local_identity: AgentIdentity::generate(), connections: Vec::new() } }

  fn local_address(&self) -> Self::Address { self.local_identity }

  fn broadcast(&mut self, envelope: Envelope<Self>) {
    for sender in self.connections.iter_mut() {
      sender.send(envelope.clone());
    }
  }
}

impl Default for InMemoryTransport {
  fn default() -> Self { Self::new() }
}
