use std::hash::Hash;

use crate::handler::{Envelope, Message, Package};

#[cfg(feature = "in-memory")] pub mod memory;

#[cfg(feature = "tcp")] pub mod tcp;

pub trait Generateable {
  fn generate() -> Self;
}

#[derive(Debug)]
pub struct Connection<N: Network> {
  pub address: N::Address,
  pub network: N,
}

impl<N: Network> Connection<N> {
  pub fn new(address: N::Address) -> Self {
    let channel = N::new();
    Self { address, network: channel }
  }

  pub fn join(&self) -> Self {
    let channel = self.network.join();
    Self { address: self.address, network: channel }
  }
}

pub trait Network: Send + Sync + Sized + 'static {
  type Address: Generateable
    + Copy
    + Send
    + Sync
    + PartialEq
    + Eq
    + Hash
    + std::fmt::Debug
    + std::fmt::Display;
  type Payload: Message + Clone + std::fmt::Debug + Package<Self::Payload>;

  fn new() -> Self;
  fn join(&self) -> Self;
  fn send(&self, envelope: Envelope<Self>) -> impl std::future::Future<Output = ()> + Send;
  fn receive(&mut self) -> impl std::future::Future<Output = Option<Envelope<Self>>> + Send;
}
