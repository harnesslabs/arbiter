use std::{any::Any, future::Future, ops::Deref, pin::Pin};

use crate::handler::Message;

pub mod memory;

pub trait Runtime: 'static {
  type Output<T>;
  fn wrap<T: 'static>(value: T) -> Self::Output<T>;
}

pub struct SyncRuntime;

impl Runtime for SyncRuntime {
  type Output<T> = T;

  fn wrap<T: 'static>(value: T) -> Self::Output<T> { value }
}

pub struct AsyncRuntime;

impl Runtime for AsyncRuntime {
  type Output<T> = Pin<Box<dyn Future<Output = T>>>;

  fn wrap<T: 'static>(value: T) -> Self::Output<T> { Box::pin(async move { value }) }
}

/// Low-level transport mechanism (TCP, Bluetooth, in-memory, etc.)
pub trait Transport<R: Runtime>: Sized + 'static {
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
  fn send(&mut self, envelope: Envelope<Self, R>) -> R::Output<Result<(), Self::Error>>;

  /// Poll for incoming envelopes
  fn poll(&mut self) -> R::Output<Vec<Envelope<Self, R>>>;

  /// Get the local address for this transport
  fn local_address(&self) -> Self::Address;
}

// // TODO: Should organize this better so we can have shared parts of the trait between both (e.g.,
// // the address, payload, error, etc.) but then make the methods async.
// pub trait AsyncTransport: Transport {
//   async fn send_async(&mut self, envelope: Envelope<Self>) -> Result<(), Self::Error>;
// }

#[derive(Clone)]
pub enum SendTarget<T: Transport<R>, R: Runtime> {
  Address(T::Address),
  Broadcast,
}

impl<T: Transport<R>, R: Runtime> PartialEq for SendTarget<T, R> {
  fn eq(&self, other: &Self) -> bool {
    match (self, other) {
      (Self::Address(a), Self::Address(b)) => a == b,
      (Self::Broadcast, Self::Broadcast) => true,
      _ => false,
    }
  }
}

#[derive(Clone)]
pub struct Envelope<T: Transport<R>, R: Runtime> {
  pub from:    T::Address,
  pub to:      SendTarget<T, R>,
  pub payload: T::Payload,
}

impl<T: Transport<R>, R: Runtime> Envelope<T, R> {
  pub fn new(from: T::Address, to: SendTarget<T, R>, payload: T::Payload) -> Self {
    Self { from, to, payload }
  }
}
