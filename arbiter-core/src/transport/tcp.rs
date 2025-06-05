#![cfg(feature = "tcp")]

use std::{
  io::{Read, Write},
  net::{SocketAddr, TcpStream},
};

use crate::{
  handler::Envelope,
  transport::{AsyncRuntime, Transport},
};

pub struct TcpTransport {
  local_identity: SocketAddr,
  connections:    Vec<TcpStream>,
}

impl Transport for TcpTransport {
  type Address = SocketAddr;
  type Error = String;
  type Payload = Vec<u8>;
  type Runtime = AsyncRuntime;

  // TODO: This should be configurable.
  fn new() -> Self {
    Self { local_identity: SocketAddr::from(([127, 0, 0, 1], 0)), connections: Vec::new() }
  }

  fn local_address(&self) -> Self::Address { self.local_identity }

  fn receive(&mut self) -> Option<Envelope<Self>> {
    let mut stream = TcpStream::connect(self.local_identity).unwrap();
    let mut buffer = Vec::new();
    stream.read_to_end(&mut buffer).unwrap();
    Some(Envelope::package(buffer))
  }

  fn broadcast(&mut self, envelope: Envelope<Self>) {
    for stream in &mut self.connections {
      stream.write_all(&envelope.payload).unwrap();
    }
  }
}
