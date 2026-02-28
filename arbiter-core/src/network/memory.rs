use std::{
  any::{Any, TypeId},
  fmt::Debug,
  sync::Arc,
};

use crate::{
  handler::{Envelope, Message},
  network::{Generateable, Network},
};

// ── InMemoryEnvelope ───────────────────────────────────────────────

#[derive(Clone)]
pub struct InMemoryEnvelope {
  type_id: TypeId,
  payload: Arc<dyn Message>,
}

impl Debug for InMemoryEnvelope {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "InMemoryEnvelope {{ type_id: {:?} }}", self.type_id)
  }
}

impl Envelope for InMemoryEnvelope {
  fn type_id(&self) -> TypeId {
    self.type_id
  }

  fn wrap<M: Message>(message: M) -> Self {
    Self { type_id: TypeId::of::<M>(), payload: Arc::new(message) }
  }

  fn downcast<M: Message>(&self) -> Option<impl std::ops::Deref<Target = M> + '_> {
    (self.payload.as_ref() as &dyn Any).downcast_ref::<M>()
  }
}

// ── InMemory network ───────────────────────────────────────────────

#[derive(Debug)]
pub struct InMemory {
  pub(crate) sender: tokio::sync::broadcast::Sender<InMemoryEnvelope>,
  pub(crate) receiver: tokio::sync::broadcast::Receiver<InMemoryEnvelope>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InMemoryAddress(u64);

impl std::fmt::Display for InMemoryAddress {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "actor-{}", self.0)
  }
}

impl Generateable for InMemoryAddress {
  fn generate() -> Self {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    Self(COUNTER.fetch_add(1, Ordering::Relaxed))
  }
}

impl Network for InMemory {
  type Address = InMemoryAddress;
  type Envelope = InMemoryEnvelope;

  fn new() -> Self {
    let (sender, receiver) = tokio::sync::broadcast::channel(1024);
    Self { sender, receiver }
  }

  fn join(&self) -> Self {
    let (sender, receiver) = (self.sender.clone(), self.sender.subscribe());
    Self { sender, receiver }
  }

  async fn send(&self, envelope: InMemoryEnvelope) {
    self.sender.send(envelope).unwrap();
  }

  async fn receive(&mut self) -> Option<InMemoryEnvelope> {
    self.receiver.recv().await.ok()
  }
}
