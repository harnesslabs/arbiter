use std::{future::Future, hash::Hash, pin::Pin};

use crate::handler::{Envelope, Message, Package};

#[cfg(feature = "in-memory")] pub mod memory;

#[cfg(feature = "tcp")] pub mod tcp;

pub trait Generateable {
  fn generate() -> Self;
}

#[derive(Debug)]
pub struct Connection<T: Transport> {
  pub address:   T::Address,
  pub transport: T,
}

impl<T: Transport> Connection<T> {
  pub fn new(address: T::Address) -> Self {
    let channel = T::spawn();
    Self { address, transport: channel }
  }

  pub fn join(&self) -> Self {
    let channel = self.transport.join();
    Self { address: self.address, transport: channel }
  }
}

pub trait Transport: Send + Sync + Sized + 'static {
  type Address: Generateable
    + Copy
    + Send
    + Sync
    + PartialEq
    + Eq
    + Hash
    + std::fmt::Debug
    + std::fmt::Display;
  type Payload: Clone + Message + Package<Self::Payload>;

  fn spawn() -> Self;
  fn join(&self) -> Self;
  async fn send(&self, envelope: Envelope<Self>);
  async fn receive(&mut self) -> Option<Envelope<Self>>;
}
