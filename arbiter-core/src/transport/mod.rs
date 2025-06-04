use std::{future::Future, hash::Hash, ops::Deref, pin::Pin, sync::Arc};

use crate::handler::Message;

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

// TODO: This should be possible to implement properly and we might be able to fix the "unpack/pack"
// stuff more nicely this way.
pub trait Payload: Clone + Message + Deref<Target = dyn Message> {}

impl Payload for Arc<dyn Message> {}

/// Low-level transport mechanism (TCP, Bluetooth, in-memory, etc.)
// TODO: The transport needs to be able to handle `Serializable` data, and `Any` data.
pub trait Transport: Sized + 'static {
  type Runtime: Runtime;

  /// Transport-specific address type (SocketAddr, BluetoothAddr, AgentId, etc.)
  type Address: Copy + Send + Sync + PartialEq + Eq + Hash + std::fmt::Debug + std::fmt::Display;

  /// Transport-specific payload type
  type Payload: Payload;

  /// Transport-specific error type
  // TODO: Make this a real error type
  type Error;

  /// Create a new transport
  fn new() -> Self;

  /// Send an envelope via this transport
  fn send(
    &mut self,
    payload: Self::Payload,
  ) -> <Self::Runtime as Runtime>::Output<Result<(), Self::Error>>;

  /// Get the local address for this transport
  fn local_address(&self) -> Self::Address;
}
