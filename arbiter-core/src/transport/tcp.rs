#![cfg(feature = "tcp")]

use std::net::SocketAddr;

use crate::transport::{AsyncRuntime, Transport};

pub struct TcpTransport {
  local_identity: SocketAddr,
}

impl Transport for TcpTransport {
  type Address = SocketAddr;
  type Error = String;
  type Payload = Vec<u8>;
  type Runtime = AsyncRuntime;

  fn new() -> Self { Self { local_identity: todo!() } }

  fn local_address(&self) -> Self::Address { todo!() }
}
