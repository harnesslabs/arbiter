//! Networking abstractions for the Arbiter framework.
//!
//! This module defines the [`Network`] and [`Socket`] traits, allowing actors
//! to communicate over various transports (e.g., in-memory channels, TCP streams).

use std::{any::TypeId, fmt::Debug, future::Future, hash::Hash};

use crate::handler::Envelope;

#[cfg(feature = "in-memory")] pub mod memory;

#[cfg(feature = "tcp")] pub mod tcp;

/// Defines a network backend capable of spawning and routing to [`Socket`]s.
pub trait Network: Sized + 'static {
  /// The type of socket created by this network.
  type Socket: Socket;

  /// Creates a new network instance.
  fn new() -> Self;

  /// Creates a new socket connected to this network.
  fn connect(&mut self) -> Self::Socket;

  /// Subscribes the given socket address to messages of the specified `TypeId`.
  fn subscribe(&self, address: <Self::Socket as Socket>::Address, type_id: TypeId);
}

/// An endpoint for sending and receiving messages over a [`Network`].
pub trait Socket: Send + 'static {
  /// The envelope type used by this socket to carry messages.
  type Envelope: Envelope;
  /// The abstract address type used to route messages to this socket.
  type Address: Copy + Send + Sync + PartialEq + Eq + Hash + Debug + std::fmt::Display + 'static;

  /// Returns the network address of this socket.
  fn address(&self) -> Self::Address;

  /// Sends an envelope to its destination through the network.
  fn send(&self, envelope: Self::Envelope) -> impl Future<Output = ()> + Send;

  /// Receives the next incoming envelope from the network.
  fn receive(&mut self) -> impl Future<Output = Option<Self::Envelope>> + Send;
}
