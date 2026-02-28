// TODO (autoparallel): This is not a functional implementation

use std::{
  any::TypeId,
  fmt::Debug,
  net::{SocketAddr, TcpStream},
  ops::Deref,
};

use serde::{Deserialize, Serialize};

use crate::{
  handler::{Envelope, Message, Package, Unpackage},
  network::{Generateable, Network},
};

// ── TcpEnvelope ────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct TcpEnvelope {
  type_id: TypeId,
  payload: Vec<u8>,
}

impl Envelope for TcpEnvelope {
  fn type_id(&self) -> TypeId {
    self.type_id
  }
}

impl<M> Package<M> for TcpEnvelope
where
  M: Message + Serialize,
{
  fn package(message: M) -> Self {
    Self { type_id: TypeId::of::<M>(), payload: serde_json::to_vec(&message).unwrap() }
  }
}

impl<M> Unpackage<M> for TcpEnvelope
where
  M: Message + for<'de> Deserialize<'de>,
{
  fn unpackage(&self) -> Option<impl Deref<Target = M>> {
    serde_json::from_slice(&self.payload).ok().map(Box::new)
  }
}

// ── TCP network ────────────────────────────────────────────────────

impl Generateable for SocketAddr {
  fn generate() -> Self {
    Self::from(([127, 0, 0, 1], 0))
  }
}

impl Network for TcpStream {
  type Address = SocketAddr;
  type Envelope = TcpEnvelope;

  fn new() -> Self {
    TcpStream::connect(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap()
  }

  fn join(&self) -> Self {
    self.try_clone().unwrap()
  }

  async fn send(&self, _envelope: TcpEnvelope) {
    todo!()
  }

  async fn receive(&mut self) -> Option<TcpEnvelope> {
    todo!()
  }
}
