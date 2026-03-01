//! Core components and abstractions for the Arbiter actor framework.
//!
//! This crate provides the foundational types and traits for building
//! actor-based systems with pluggable networking and lifecycle management.

#![allow(refining_impl_trait)]

pub mod actor;
pub mod error;
pub mod handler;
pub mod network;
pub mod processor;
pub mod runtime;

/// A convenient prelude for bringing the most common Arbiter traits and types into scope.
pub mod prelude {
  pub use crate::{
    actor::LifeCycle,
    handler::{Handler, Message},
    network::{Network, Socket},
  };
}

/// Common test fixtures and mock actors used for testing.
#[cfg(any(test, feature = "fixtures"))]
pub mod fixtures {
  use crate::prelude::*;

  /// A simple ping message.
  #[derive(Debug, Clone)]
  #[cfg_attr(feature = "tcp", derive(serde::Serialize, serde::Deserialize))]
  pub struct Ping;

  /// A simple pong message.
  #[derive(Debug, Clone)]
  #[cfg_attr(feature = "tcp", derive(serde::Serialize, serde::Deserialize))]
  pub struct Pong;

  /// A simple actor that counts every message it receives.
  /// Handles both [`Ping`] and [`Pong`], incrementing `count` for each.
  /// Snapshot is the current count.
  #[derive(Debug, Clone)]
  #[cfg_attr(feature = "tcp", derive(serde::Serialize, serde::Deserialize))]
  pub struct Counter {
    pub count: usize,
  }

  impl LifeCycle for Counter {
    type Snapshot = usize;
    type StartMessage = ();
    type StopMessage = ();

    fn on_start(&mut self) -> Self::StartMessage {}

    fn on_stop(&mut self) -> Self::StopMessage {}

    fn snapshot(&self) -> Self::Snapshot {
      self.count
    }
  }

  impl Handler<Ping> for Counter {
    type Reply = ();

    fn handle(&mut self, _message: &Ping) -> Option<Self::Reply> {
      self.count += 1;
      tracing::debug!(count = self.count, "Counter received Ping");
      None
    }
  }

  impl Handler<Pong> for Counter {
    type Reply = ();

    fn handle(&mut self, _message: &Pong) -> Option<Self::Reply> {
      self.count += 1;
      tracing::debug!(count = self.count, "Counter received Pong");
      None
    }
  }

  /// Initiates a ping-pong exchange. Sends a [`Ping`] on start, then for
  /// each [`Pong`] received, increments count and replies with [`Ping`].
  /// Stops itself when count reaches `max_count`.
  ///
  /// Snapshot is the current count — ideal for stream-based testing.
  #[derive(Debug, Clone)]
  #[cfg_attr(feature = "tcp", derive(serde::Serialize, serde::Deserialize))]
  pub struct PingPlayer {
    pub count: usize,
    pub max_count: usize,
  }

  impl LifeCycle for PingPlayer {
    type Snapshot = usize;
    type StartMessage = Ping;
    type StopMessage = ();

    fn on_start(&mut self) -> Self::StartMessage {
      Ping
    }

    fn on_stop(&mut self) -> Self::StopMessage {}

    fn snapshot(&self) -> Self::Snapshot {
      self.count
    }

    fn should_stop(&self) -> bool {
      self.count >= self.max_count
    }
  }

  impl Handler<Pong> for PingPlayer {
    type Reply = Ping;

    fn handle(&mut self, _message: &Pong) -> Option<Self::Reply> {
      self.count += 1;
      Some(Ping)
    }
  }

  /// Simple responder: replies [`Pong`] to every [`Ping`].
  /// No meaningful state — just an echo partner.
  #[derive(Debug, Clone)]
  #[cfg_attr(feature = "tcp", derive(serde::Serialize, serde::Deserialize))]
  pub struct PongPlayer;

  impl LifeCycle for PongPlayer {
    type Snapshot = ();
    type StartMessage = ();
    type StopMessage = ();

    fn on_start(&mut self) -> Self::StartMessage {}

    fn on_stop(&mut self) -> Self::StopMessage {}

    fn snapshot(&self) -> Self::Snapshot {}
  }

  impl Handler<Ping> for PongPlayer {
    type Reply = Pong;

    fn handle(&mut self, _message: &Ping) -> Option<Self::Reply> {
      Some(Pong)
    }
  }
}
