// TODO (autoparallel): This is not a functional implementation

use std::net::{SocketAddr, TcpStream};

use crate::{
  handler::Envelope,
  network::{Generateable, Network},
};

// TODO
impl Generateable for SocketAddr {
  fn generate() -> Self {
    Self::from(([127, 0, 0, 1], 0))
  }
}

impl Network for TcpStream {
  type Address = SocketAddr;
  type Payload = Vec<u8>;

  fn new() -> Self {
    TcpStream::connect(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap()
  }

  fn join(&self) -> Self {
    self.try_clone().unwrap()
  }

  async fn send(&self, _envelope: Envelope<Self>) {
    todo!()
  }

  async fn receive(&mut self) -> Option<Envelope<Self>> {
    todo!()
  }
}
