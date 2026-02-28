//! TCP network implementation for distributed actor communication.
//!
//! (Currently a skeleton/stub).

use std::any::TypeId;

use crate::{
  handler::{Envelope, Message},
  network::{Network, Socket},
};

/// An envelope containing a message for the `TcpStream` network.
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

/// A network implementation that communicates via TCP streams.
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

/// The socket endpoint assigned to an actor on the `TcpStream` network.
pub struct TcpSocket;

impl Socket for TcpSocket {
  type Address = std::net::SocketAddr;
  type Envelope = TcpEnvelope;

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
