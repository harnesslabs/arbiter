use crate::{agent::Agent, environment::Environment, network::Network};

pub struct Runtime<N: Network, E: Environment> {
  pub network: N,
  pub environment: E,
}

impl<N: Network, E: Environment> Runtime<N, E> {
  pub fn new() -> Self {
    let network = N::new();
    let environment = E::new();
    Self { network, environment }
  }

  //   pub fn add_agent<A: Agent<>(&mut self, agent: A) -> String {
  //     let agent_id = self.network.add_agent(agent);

  //     agent_id
  //   }
}
