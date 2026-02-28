use std::{
  any::{Any, TypeId},
  collections::HashMap,
  fmt::Debug,
  sync::Arc,
};

use tokio::sync::mpsc;

use crate::{
  handler::{Envelope, Message},
  network::{Network, Socket},
};

// ── Router message ─────────────────────────────────────────────────

enum RouterMessage {
  Register(InMemoryAddress, mpsc::UnboundedSender<InMemoryEnvelope>),
  Subscribe(InMemoryAddress, TypeId),
  Dispatch(InMemoryEnvelope),
}

// ── Router background task ─────────────────────────────────────────

async fn router(mut rx: mpsc::UnboundedReceiver<RouterMessage>) {
  let mut inboxes: HashMap<InMemoryAddress, mpsc::UnboundedSender<InMemoryEnvelope>> =
    HashMap::new();
  let mut routes: HashMap<TypeId, Vec<InMemoryAddress>> = HashMap::new();

  while let Some(msg) = rx.recv().await {
    match msg {
      RouterMessage::Register(addr, tx) => {
        inboxes.insert(addr, tx);
      },
      RouterMessage::Subscribe(addr, tid) => {
        routes.entry(tid).or_default().push(addr);
      },
      RouterMessage::Dispatch(envelope) => {
        if let Some(addrs) = routes.get(&Envelope::type_id(&envelope)) {
          for addr in addrs {
            if let Some(tx) = inboxes.get(addr) {
              let _ = tx.send(envelope.clone());
            }
          }
        }
      },
    }
  }
}

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

// ── InMemoryAddress ────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InMemoryAddress(u64);

impl std::fmt::Display for InMemoryAddress {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "actor-{}", self.0)
  }
}

impl InMemoryAddress {
  fn generate() -> Self {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    Self(COUNTER.fetch_add(1, Ordering::Relaxed))
  }
}

// ── InMemory network ───────────────────────────────────────────────

pub struct InMemory {
  router_tx: mpsc::UnboundedSender<RouterMessage>,
}

impl Debug for InMemory {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "InMemory")
  }
}

impl InMemory {
  /// Send a message into the network (for test injection via `runtime.network()`).
  pub fn send(&self, envelope: InMemoryEnvelope) {
    let _ = self.router_tx.send(RouterMessage::Dispatch(envelope));
  }
}

impl Network for InMemory {
  type Socket = InMemorySocket;

  fn new() -> Self {
    let (router_tx, router_rx) = mpsc::unbounded_channel();
    tokio::spawn(router(router_rx));
    Self { router_tx }
  }

  fn connect(&mut self) -> InMemorySocket {
    let address = InMemoryAddress::generate();
    let (inbox_tx, inbox_rx) = mpsc::unbounded_channel();
    let _ = self.router_tx.send(RouterMessage::Register(address, inbox_tx));
    InMemorySocket { address, router_tx: self.router_tx.clone(), inbox_rx }
  }

  fn subscribe(&self, address: InMemoryAddress, type_id: TypeId) {
    let _ = self.router_tx.send(RouterMessage::Subscribe(address, type_id));
  }
}

// ── InMemorySocket ─────────────────────────────────────────────────

pub struct InMemorySocket {
  address: InMemoryAddress,
  router_tx: mpsc::UnboundedSender<RouterMessage>,
  inbox_rx: mpsc::UnboundedReceiver<InMemoryEnvelope>,
}

impl Debug for InMemorySocket {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "InMemorySocket {{ address: {} }}", self.address)
  }
}

impl Socket for InMemorySocket {
  type Envelope = InMemoryEnvelope;
  type Address = InMemoryAddress;

  fn address(&self) -> InMemoryAddress {
    self.address
  }

  async fn send(&self, envelope: InMemoryEnvelope) {
    let _ = self.router_tx.send(RouterMessage::Dispatch(envelope));
  }

  async fn receive(&mut self) -> Option<InMemoryEnvelope> {
    self.inbox_rx.recv().await
  }
}
