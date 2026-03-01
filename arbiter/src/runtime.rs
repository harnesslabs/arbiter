//! High-level execution environment for the Arbiter framework.
//!
//! The [`Runtime`] manages the underlying network and provides the entry point
//! for spawning and processing actors.

use crate::{
  actor::{Actor, LifeCycle},
  network::{Network, Socket},
  processor::Processing,
};

/// The Arbiter execution environment.
///
/// It holds the [`Network`] implementation and acts as a factory for actors,
/// connecting them to the network and transitioning them into processing tasks.
pub struct Runtime<N: Network> {
  network: N,
}

impl<N: Network> Runtime<N> {
  /// Creates a new `Runtime` wrapped around a fresh `Network`.
  #[must_use]
  #[allow(clippy::new_without_default)]
  pub fn new() -> Self { Self { network: N::new() } }

  /// Spawns a new actor into the runtime, connecting it to the network.
  ///
  /// The returned [`Actor`] can be further configured with handlers and a name
  /// before being turned into a running task via [`Runtime::process`].
  pub fn spawn<L: LifeCycle>(&mut self, lifecycle: L) -> Actor<L, N> {
    let socket = self.network.connect();
    Actor::new(lifecycle, socket)
  }

  /// Creates a raw socket connected to the network without spawning an actor.
  ///
  /// This is useful for injecting messages from external sources (e.g., stdin, external APIs)
  /// directly into the network.
  pub fn socket(&mut self) -> N::Socket { self.network.connect() }

  /// Subscribes the actor's handlers to the network, then starts processing.
  ///
  /// Returns a [`Processing`] handle to observe and control the actor.
  pub fn process<L: LifeCycle>(&self, actor: Actor<L, N>) -> Processing<Actor<L, N>, L, N> {
    for &type_id in actor.handlers.keys() {
      self.network.subscribe(actor.socket.address(), type_id);
    }
    actor.into_processing()
  }

  /// Returns a reference to the underlying network.
  #[must_use]
  pub const fn network(&self) -> &N { &self.network }
}
