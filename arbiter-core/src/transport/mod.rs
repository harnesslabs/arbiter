use std::{any::Any, future::Future, ops::Deref, pin::Pin};

use serde::Deserialize;

use crate::handler::{Message, UnpackageMessage};

pub mod memory;
pub mod tcp;

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
  type Payload: Clone + Message + AsRef<dyn Any>;

  /// Transport-specific error type
  // TODO: Make this a real error type
  type Error;

  /// Create a new transport
  fn new() -> Self;

  /// Send an envelope via this transport
  fn send(&mut self, payload: Self::Payload) -> R::Output<Result<(), Self::Error>>;

  /// Poll for incoming envelopes
  fn poll(&mut self) -> R::Output<Vec<Self::Payload>>;

  /// Get the local address for this transport
  fn local_address(&self) -> Self::Address;
}
