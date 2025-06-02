pub mod memory;

/// Low-level transport mechanism (TCP, Bluetooth, in-memory, etc.)
pub trait TransportLayer: Sized {
  /// Transport-specific address type (SocketAddr, BluetoothAddr, AgentId, etc.)
  type Address: Clone + Send + Sync;

  /// Transport-specific payload type
  type Payload;

  /// Transport-specific error type
  // TODO: Make this a real error type
  type Error;

  /// Send an envelope via this transport
  fn send(&mut self, envelope: Envelope<Self>) -> Result<(), Self::Error>;

  /// Poll for incoming envelopes
  fn poll(&mut self) -> Vec<Envelope<Self>>;

  /// Get the local address for this transport
  fn local_address(&self) -> Self::Address;
}

/// Agent's capability to handle a specific transport layer
pub trait Transport<T: TransportLayer> {
  /// Check if this agent can handle the given envelope
  fn can_handle_envelope(&self, envelope: &Envelope<T>) -> bool;
}

pub enum SendTarget<T: TransportLayer> {
  Address(T::Address),
  Broadcast,
}

pub struct Envelope<T: TransportLayer> {
  from:    T::Address,
  to:      SendTarget<T>,
  payload: T::Payload,
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
