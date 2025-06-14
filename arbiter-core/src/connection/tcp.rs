// TODO (autoparallel): This is not a functional implementation

use std::net::{SocketAddr, TcpStream};

use crate::{
  connection::{Generateable, Transport},
  handler::Envelope,
};

// TODO
impl Generateable for SocketAddr {
  fn generate() -> Self { SocketAddr::from(([127, 0, 0, 1], 0)) }
}

impl Transport for TcpStream {
  type Address = SocketAddr;
  type Payload = Vec<u8>;

  fn spawn() -> Self {
    let stream = TcpStream::connect(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap();
    stream
  }

  fn join(&self) -> Self { self.try_clone().unwrap() }

  fn send(&self, envelope: Envelope<Self>) { todo!() }

  fn receive(&self) -> Option<Envelope<Self>> { todo!() }
}
