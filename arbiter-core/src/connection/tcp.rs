#![cfg(feature = "tcp")]

use std::{
  net::{SocketAddr, TcpStream},
  sync::Arc,
};

use crate::{agent::Controller, connection::Connection, handler::Envelope};

pub struct TcpConnection {
  pub(crate) address:     SocketAddr,
  pub(crate) connections: Vec<TcpStream>,
}

impl Connection for TcpConnection {
  type Address = SocketAddr;
  type Payload = Vec<u8>;
  type Receiver = TcpStream;
  type Sender = TcpStream;

  // TODO: This should be configurable.
  fn new() -> Self {
    Self { address: SocketAddr::from(([127, 0, 0, 1], 0)), connections: Vec::new() }
  }

  fn address(&self) -> Self::Address { self.address }

  fn get_sender(&self) -> Self::Sender { todo!() }

  fn send(&mut self, envelope: Envelope<Self>) { todo!() }

  fn receive(&mut self) -> Option<Envelope<Self>> { todo!() }
}
