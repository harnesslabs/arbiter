// TODO (autoparallel): This is not a functional implementation

use std::{
  any::TypeId,
  net::{SocketAddr, TcpStream},
};

use crate::{
  handler::{Envelope, Message},
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

  fn wrap<M: Message>(_message: M) -> Self {
    todo!("TCP serialization not yet implemented")
  }

  fn downcast<M: Message>(&self) -> Option<impl std::ops::Deref<Target = M> + '_> {
    todo!("TCP deserialization not yet implemented");
    #[allow(unreachable_code)]
    None::<&M>
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
