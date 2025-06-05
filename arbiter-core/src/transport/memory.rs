use std::sync::Arc;

use super::Transport;
use crate::{
  agent::{AgentIdentity, Controller},
  handler::{Envelope, Message},
  transport::SyncRuntime,
};

// TODO: Perhaps Transport really should be a struct.
/// In-memory transport that preserves current Runtime behavior
pub struct InMemoryTransport {
  local_identity:    AgentIdentity,
  receiver:          flume::Receiver<Envelope<Self>>,
  pub(crate) sender: flume::Sender<Envelope<Self>>,
  connections:       Vec<InMemoryConnection>,
}

pub struct InMemoryConnection {
  controller: Arc<Controller>,
  sender:     flume::Sender<Envelope<InMemoryTransport>>,
}

impl Transport for InMemoryTransport {
  type Address = AgentIdentity;
  type Error = String;
  type Payload = Arc<dyn Message>;
  type Runtime = SyncRuntime;

  fn new() -> Self {
    let (tx, rx) = flume::unbounded();
    Self {
      local_identity: AgentIdentity::generate(),
      receiver:       rx,
      sender:         tx,
      connections:    Vec::new(),
    }
  }

  fn local_address(&self) -> Self::Address { self.local_identity }

  fn receive(&mut self) -> Option<Envelope<Self>> {
    // dbg!("recieve");
    self.receiver.try_recv().ok()
  }

  fn broadcast(&mut self, envelope: Envelope<Self>) {
    for connection in self.connections.iter_mut() {
      connection.sender.send(envelope.clone());
      connection.controller.condvar.notify_one();
    }
  }
}

impl Default for InMemoryTransport {
  fn default() -> Self { Self::new() }
}
