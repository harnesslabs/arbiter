#![cfg(feature = "tcp")]

use std::{
  io::Write,
  net::{SocketAddr, TcpStream},
};

use crate::{
  connection::{Connection, Receiver, Sender},
  handler::Envelope,
};

pub struct Tcp {
  pub(crate) stream: TcpStream,
}

impl Sender for TcpStream {
  type Connection = Tcp;

  fn send(&self, envelope: Envelope<Self::Connection>) { todo!() }
}

impl Receiver for TcpStream {
  type Connection = Tcp;

  fn receive(&self) -> Option<Envelope<Self::Connection>> { todo!() }
}

impl Connection for Tcp {
  type Address = SocketAddr;
  type Payload = Vec<u8>;
  type Receiver = TcpStream;
  type Sender = TcpStream;

  // TODO: This should be configurable.
  fn new() -> Self {
    let stream = TcpStream::connect(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap();
    Self { stream }
  }

  fn address(&self) -> Self::Address { self.stream.peer_addr().unwrap() }

  fn create_outbound_connection(&self) -> (Self::Address, Self::Sender) {
    let stream = TcpStream::connect(self.address()).unwrap();
    (self.address(), stream)
  }

  fn sender(&mut self) -> &mut Self::Sender { &mut self.stream }

  fn receiver(&mut self) -> &mut Self::Receiver { &mut self.stream }
}
