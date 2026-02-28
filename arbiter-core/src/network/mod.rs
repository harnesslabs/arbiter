use std::{any::TypeId, fmt::Debug, future::Future, hash::Hash};

use crate::handler::Envelope;

#[cfg(feature = "in-memory")]
pub mod memory;

#[cfg(feature = "tcp")]
pub mod tcp;

pub trait Network: Sized + 'static {
  type Socket: Socket;

  fn new() -> Self;
  fn connect(&mut self) -> Self::Socket;
  fn subscribe(&self, address: <Self::Socket as Socket>::Address, type_id: TypeId);
}

pub trait Socket: Send + 'static {
  type Envelope: Envelope;
  type Address: Copy + Send + Sync + PartialEq + Eq + Hash + Debug + std::fmt::Display + 'static;

  fn address(&self) -> Self::Address;
  fn send(&self, envelope: Self::Envelope) -> impl Future<Output = ()> + Send;
  fn receive(&mut self) -> impl Future<Output = Option<Self::Envelope>> + Send;
}
