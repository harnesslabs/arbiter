#![allow(refining_impl_trait)]

pub mod actor;
pub mod error;
pub mod handler;
pub mod network;
pub mod processor;
pub mod runtime;

pub mod prelude {
  pub use crate::{
    actor::LifeCycle,
    handler::{HandleResult, Handler, Message},
    network::Network,
  };
}

#[cfg(any(test, feature = "fixtures"))]
pub mod fixtures {
  use crate::prelude::*;

  // ── Messages ─────────────────────────────────────────────────────

  #[derive(Debug, Clone)]
  pub struct Ping;

  #[derive(Debug, Clone)]
  pub struct Pong;

  // ── Counter ──────────────────────────────────────────────────────
  //
  // A simple actor that counts every message it receives.
  // Handles both Ping and Pong, incrementing `count` for each.
  // Snapshot is the current count.

  #[derive(Debug, Clone)]
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

    fn handle(&mut self, _message: &Ping) {
      self.count += 1;
      tracing::debug!(count = self.count, "Counter received Ping");
    }
  }

  impl Handler<Pong> for Counter {
    type Reply = ();

    fn handle(&mut self, _message: &Pong) {
      self.count += 1;
      tracing::debug!(count = self.count, "Counter received Pong");
    }
  }

  // ── PingPlayer ───────────────────────────────────────────────────
  //
  // Initiates a ping-pong exchange. Sends a Ping on start, then for
  // each Pong received, increments count and replies with Ping.
  // Stops itself when count reaches max_count.
  // Snapshot is the current count — ideal for stream-based testing.

  #[derive(Debug, Clone)]
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
  }

  impl Handler<Pong> for PingPlayer {
    type Reply = Ping;

    fn handle(&mut self, _message: &Pong) -> HandleResult<Self::Reply> {
      if self.count == self.max_count {
        HandleResult::Stop
      } else {
        self.count += 1;
        HandleResult::Message(Ping)
      }
    }
  }

  // ── PongPlayer ───────────────────────────────────────────────────
  //
  // Simple responder: replies Pong to every Ping.
  // No meaningful state — just an echo partner.

  #[derive(Debug, Clone)]
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

    fn handle(&mut self, _message: &Ping) -> Self::Reply {
      Pong
    }
  }
}
