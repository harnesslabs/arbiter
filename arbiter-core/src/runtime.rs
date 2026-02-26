use std::sync::Arc;

use tokio::sync::Mutex;

use crate::{
  agent::{Agent, LifeCycle},
  environment::Environment,
  network::Network,
};

pub struct Runtime<N: Network, E: Environment = ()> {
  pub network: N,
  pub environment: Arc<Mutex<E>>,
}

impl<N: Network, E: Environment> Runtime<N, E> {
  pub fn new() -> Self {
    let network = N::new();
    let environment = Arc::new(Mutex::new(E::new()));
    Self { network, environment }
  }

  pub fn spawn<L: LifeCycle>(&self, agent: L) -> Agent<L, N, E> {
    Agent::join(agent, &self.network, self.environment.clone())
  }
}
