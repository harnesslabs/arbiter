use std::{
  any::{Any, TypeId},
  ops::Deref,
};

use crate::handler::Message;

pub mod memory;

/// Low-level transport mechanism (TCP, Bluetooth, in-memory, etc.)
pub trait Transport: Sized + 'static {
  /// Transport-specific address type (SocketAddr, BluetoothAddr, AgentId, etc.)
  type Address: Copy + Send + Sync + PartialEq + Eq;

  /// Transport-specific payload type
  type Payload: Clone + Deref<Target = dyn Any> + From<Box<dyn Message>>;

  /// Transport-specific error type
  // TODO: Make this a real error type
  type Error;

  /// Create a new transport
  fn new() -> Self;

  /// Send an envelope via this transport
  fn send(&mut self, envelope: Envelope<Self>) -> Result<(), Self::Error>;

  /// Poll for incoming envelopes
  fn poll(&mut self) -> Vec<Envelope<Self>>;

  /// Get the local address for this transport
  fn local_address(&self) -> Self::Address;
}

#[derive(Clone)]
pub enum SendTarget<T: Transport> {
  Address(T::Address),
  Broadcast,
}

impl<T: Transport> PartialEq for SendTarget<T> {
  fn eq(&self, other: &Self) -> bool {
    match (self, other) {
      (Self::Address(a), Self::Address(b)) => a == b,
      (Self::Broadcast, Self::Broadcast) => true,
      _ => false,
    }
  }
}

#[derive(Clone)]
pub struct Envelope<T: Transport> {
  pub from:    T::Address,
  pub to:      SendTarget<T>,
  pub payload: T::Payload,
}

impl<T: Transport> Envelope<T> {
  pub fn new(from: T::Address, to: SendTarget<T>, payload: T::Payload) -> Self {
    Self { from, to, payload }
  }
}
