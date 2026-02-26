use std::sync::Arc;

use tokio::sync::Mutex;

use crate::{
  agent::{Agent, LifeCycle},
  environment::Environment,
  handler::{Envelope, Package},
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

  pub async fn broadcast_state(&self)
  where
    N::Payload: Package<E::State>,
  {
    let state = self.environment.lock().await.get_state();
    self.network.send(Envelope::package(state)).await;
  }
}
