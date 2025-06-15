use std::sync::Arc;

use crate::{
  handler::{Envelope, Message},
  network::{Generateable, Network},
};

#[derive(Debug)]
pub struct InMemory {
  pub(crate) sender:   tokio::sync::broadcast::Sender<Envelope<Self>>,
  pub(crate) receiver: tokio::sync::broadcast::Receiver<Envelope<Self>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InMemoryAddress([u8; 32]);

impl InMemoryAddress {
  pub const fn from_bytes(bytes: [u8; 32]) -> Self { Self(bytes) }

  pub const fn as_bytes(&self) -> &[u8; 32] { &self.0 }
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
  type Payload = Arc<dyn Message>;

  fn new() -> Self {
    let (sender, receiver) = tokio::sync::broadcast::channel(1024);
    Self { sender, receiver }
  }

  fn join(&self) -> Self {
    let (sender, receiver) = (self.sender.clone(), self.sender.subscribe());
    Self { sender, receiver }
  }

  async fn send(&self, envelope: Envelope<Self>) { self.sender.send(envelope).unwrap(); }

  async fn receive(&mut self) -> Option<Envelope<Self>> { self.receiver.recv().await.ok() }
}
