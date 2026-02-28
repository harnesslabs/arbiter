use std::hash::Hash;

use crate::handler::Envelope;

#[cfg(feature = "in-memory")]
pub mod memory;

#[cfg(feature = "tcp")]
pub mod tcp;

pub trait Generateable {
  fn generate() -> Self;
}

#[derive(Debug)]
pub struct Connection<N: Network> {
  pub address: N::Address,
  pub network: N,
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
  type Envelope: Envelope;

  fn new() -> Self;
  fn join(&self) -> Self;
  fn send(&self, envelope: Self::Envelope) -> impl std::future::Future<Output = ()> + Send;
  fn receive(&mut self) -> impl std::future::Future<Output = Option<Self::Envelope>> + Send;
}
