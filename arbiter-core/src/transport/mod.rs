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

/// Unique identifier for agents across fabrics
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AgentIdentity([u8; 32]);

impl AgentIdentity {
  /// Generate a cryptographically secure agent identity
  pub fn generate() -> Self {
    // For now, use a simple counter - can be upgraded to crypto later
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(1);

    let mut bytes = [0u8; 32];
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    bytes[..8].copy_from_slice(&id.to_le_bytes());

    Self(bytes)
  }

  pub fn from_bytes(bytes: [u8; 32]) -> Self { Self(bytes) }

  pub fn as_bytes(&self) -> &[u8; 32] { &self.0 }
}

impl std::fmt::Display for AgentIdentity {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let short = &self.0[..4];
    write!(f, "agent-{:02x}{:02x}{:02x}{:02x}", short[0], short[1], short[2], short[3])
  }
}

/// Fabric identifier for connecting multiple fabrics
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FabricId([u8; 16]);

impl FabricId {
  pub fn generate() -> Self {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(1);

    let mut bytes = [0u8; 16];
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    bytes[..8].copy_from_slice(&id.to_le_bytes());

    Self(bytes)
  }

  pub fn from_bytes(bytes: [u8; 16]) -> Self { Self(bytes) }

  pub fn as_bytes(&self) -> &[u8; 16] { &self.0 }
}

impl std::fmt::Display for FabricId {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let short = &self.0[..4];
    write!(f, "fabric-{:02x}{:02x}{:02x}{:02x}", short[0], short[1], short[2], short[3])
  }
}
