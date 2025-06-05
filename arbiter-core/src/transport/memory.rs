use std::{any::Any, collections::VecDeque, rc::Rc, sync::Arc};

use super::Transport;
use crate::{agent::AgentIdentity, handler::Message, transport::SyncRuntime};

/// In-memory transport that preserves current Runtime behavior
pub struct InMemoryTransport {
  local_identity: AgentIdentity,
}

// TODO: This is a hack to get around the fact that we need to store messages in a VecDeque
// We need to find a better way to do this.
// impl From<Box<dyn Message>> for Rc<dyn Any> {
//   fn from(message: Box<dyn Message>) -> Self { Rc::new(Box::leak(message)) }
// }

impl Transport for InMemoryTransport {
  type Address = AgentIdentity;
  type Error = String;
  type Payload = Arc<dyn Message>;
  type Runtime = SyncRuntime;

  fn new() -> Self { Self { local_identity: AgentIdentity::generate() } }

  fn local_address(&self) -> Self::Address { self.local_identity }
}

impl Default for InMemoryTransport {
  fn default() -> Self { Self::new() }
}
