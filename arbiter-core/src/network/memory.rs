use std::{
  any::{Any, TypeId},
  fmt::Debug,
  ops::Deref,
  sync::Arc,
};

use crate::{
  handler::{Envelope, Message, Package, Unpackage},
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
}

impl<M: Message> Package<M> for InMemoryEnvelope {
  fn package(message: M) -> Self {
    Self { type_id: TypeId::of::<M>(), payload: Arc::new(message) }
  }
}

impl<M: Message> Unpackage<M> for InMemoryEnvelope {
  fn unpackage(&self) -> Option<impl Deref<Target = M>> {
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
pub struct InMemoryAddress([u8; 32]);

impl InMemoryAddress {
  pub const fn from_bytes(bytes: [u8; 32]) -> Self {
    Self(bytes)
  }

  pub const fn as_bytes(&self) -> &[u8; 32] {
    &self.0
  }
}

impl Generateable for InMemoryAddress {
  fn generate() -> Self {
    use std::sync::atomic::{AtomicU64, Ordering}; // Keep this for unique ID generation
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    let mut bytes = [0u8; 32];
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    bytes[..8].copy_from_slice(&id.to_le_bytes());
    Self(bytes)
  }
}

impl std::fmt::Display for InMemoryAddress {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let short = &self.0[..4];
    write!(f, "agent-{:02x}{:02x}{:02x}{:02x}", short[0], short[1], short[2], short[3])
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
