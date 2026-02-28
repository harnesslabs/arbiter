use std::any::TypeId;

use crate::{
  handler::{Envelope, Message},
  network::{Network, Socket},
};

// ── TcpEnvelope ────────────────────────────────────────────────────

#[derive(Debug)]
pub struct TcpEnvelope;

impl Envelope for TcpEnvelope {
  fn type_id(&self) -> TypeId {
    todo!()
  }

  fn wrap<M: Message>(_message: M) -> Self {
    todo!()
  }

  fn downcast<M: Message>(&self) -> Option<impl std::ops::Deref<Target = M> + '_> {
    let opt: Option<&M> = None;
    opt
  }
}

// ── Tcp network ────────────────────────────────────────────────────

pub struct TcpStream;

impl Network for TcpStream {
  type Socket = TcpSocket;

  fn new() -> Self {
    todo!()
  }

  fn connect(&mut self) -> Self::Socket {
    todo!()
  }

  fn subscribe(&self, _address: <Self::Socket as Socket>::Address, _type_id: TypeId) {
    todo!()
  }
}

// ── TcpSocket ──────────────────────────────────────────────────────

pub struct TcpSocket;

impl Socket for TcpSocket {
  type Envelope = TcpEnvelope;
  type Address = std::net::SocketAddr;

  fn address(&self) -> Self::Address {
    todo!()
  }

  async fn send(&self, _envelope: Self::Envelope) {
    todo!()
  }

  async fn receive(&mut self) -> Option<Self::Envelope> {
    todo!()
  }
}
