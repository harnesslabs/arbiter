use crate::{
  actor::{Actor, LifeCycle},
  network::Network,
};

pub struct Runtime<N: Network> {
  network: N,
}

impl<N: Network> Runtime<N> {
  pub fn new() -> Self {
    Self { network: N::new() }
  }

  pub fn spawn<L: LifeCycle>(&self, agent: L) -> Actor<L, N> {
    Actor::new(agent, &self.network)
  }
}
