use crate::{
  actor::{Actor, LifeCycle},
  network::{Network, Socket},
  processor::Processing,
};

pub struct Runtime<N: Network> {
  network: N,
}

impl<N: Network> Runtime<N> {
  pub fn new() -> Self {
    Self { network: N::new() }
  }

  pub fn spawn<L: LifeCycle>(&mut self, lifecycle: L) -> Actor<L, N> {
    let socket = self.network.connect();
    Actor::new(lifecycle, socket)
  }

  /// Subscribe the actor's handlers with the network, then start processing.
  pub fn process<L: LifeCycle>(&self, actor: Actor<L, N>) -> Processing<Actor<L, N>, L, N> {
    for &type_id in actor.handlers.keys() {
      self.network.subscribe(actor.socket.address(), type_id);
    }
    actor.into_processing()
  }

  pub fn network(&self) -> &N {
    &self.network
  }
}
