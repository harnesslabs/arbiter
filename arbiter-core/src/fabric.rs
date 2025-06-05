use std::collections::HashMap;

use crate::{
  agent::{Agent, LifeCycle, RuntimeAgent, State},
  connection::{memory::InMemory, Connection},
  handler::Package,
};

/// A generic fabric that manages agents over a specific transport layer
// TODO: Instead of storing agents, we could just store the transport components (e.g., a sender
// and an atomic for their state)
pub struct Fabric<C: Connection> {
  id:         FabricId,
  agents:     HashMap<C::Address, Box<dyn RuntimeAgent<C>>>,
  name_to_id: HashMap<String, C::Address>,
}

impl<C: Connection> Fabric<C> {
  /// Create a new fabric with the given transport
  pub fn new() -> Self {
    Self {
      id:         FabricId::generate(),
      agents:     HashMap::new(),
      name_to_id: HashMap::new(),
    }
  }

  /// Get the fabric's unique identifier
  pub fn id(&self) -> FabricId { self.id }

  /// Register an agent with the fabric
  // TODO: Give all the other agents access to the new agent's connection and vice versa.
  pub fn register_agent<A>(&mut self, mut new_agent: Agent<A, C>) -> C::Address
  where
    A: LifeCycle,
    C::Payload: Package<A::StartMessage> + Package<A::StopMessage>, {
    if let Some(name) = new_agent.name.clone() {
      if self.name_to_id.contains_key(&name) {
        panic!("Agent name '{name}' already exists");
      }
      self.name_to_id.insert(name, new_agent.address());
    }
    let id = new_agent.address();
    for agent in self.agents.values_mut() {
      // Get the new agent's inbound connection and give it to all the other agents as an outbound
      // connection.
      let (address, sender) = new_agent.transport().create_inbound_connection();
      agent.add_outbound_connection(address, sender);
      // Get the other agent's inbound connection and give it to the new agent as an outbound
      // connection.
      let (address, sender) = agent.transport().create_inbound_connection();
      new_agent.add_outbound_connection(address, sender);
    }
    self.agents.insert(id, Box::new(new_agent));
    id
  }

  /// Register an agent with a name
  pub fn register_named_agent<A>(
    &mut self,
    name: impl Into<String>,
    mut agent: Agent<A, C>,
  ) -> Result<C::Address, String>
  where
    A: LifeCycle,
    C::Payload: Package<A::StartMessage> + Package<A::StopMessage>,
  {
    agent.name = Some(name.into());
    Ok(self.register_agent(agent))
  }

  /// Register an agent and start it immediately
  pub fn spawn_agent<A>(&mut self, agent: Agent<A, C>) -> C::Address
  where
    A: LifeCycle,
    C::Payload: Package<A::StartMessage> + Package<A::StopMessage>, {
    let id = self.register_agent(agent);
    self.start_agent_by_id(id).unwrap(); // Safe since we just added it
    id
  }

  /// Register a named agent and start it immediately
  pub fn spawn_named_agent<A>(
    &mut self,
    name: impl Into<String>,
    agent: Agent<A, C>,
  ) -> Result<C::Address, String>
  where
    A: LifeCycle,
    C::Payload: Package<A::StartMessage> + Package<A::StopMessage>,
  {
    let id = self.register_named_agent(name, agent)?;
    self.start_agent_by_id(id).unwrap(); // Safe since we just added it
    Ok(id)
  }

  /// Look up agent ID by name
  pub fn agent_id_by_name(&self, name: &str) -> Option<C::Address> {
    self.name_to_id.get(name).copied()
  }

  /// Start an agent by ID
  pub fn start_agent_by_id(&mut self, agent_id: C::Address) -> Result<(), String> {
    self.agents.get_mut(&agent_id).map_or_else(
      || Err(format!("Agent with ID {agent_id} not found")),
      |agent| {
        agent.signal_start();
        Ok(())
      },
    )
  }

  /// Start an agent by name
  pub fn start_agent_by_name(&mut self, agent_name: &str) -> Result<(), String> {
    let agent_id = self.agent_id_by_name(agent_name).ok_or_else(|| {
      format!(
        "Agent '{agent_name}' not
found"
      )
    })?;
    self.start_agent_by_id(agent_id)
  }

  // TODO: This is a bit clunky. Perhaps we can combine these states.
  /// Get the state of an agent by ID
  pub fn agent_state_by_id(&self, agent_id: C::Address) -> Option<State> {
    self.agents.get(&agent_id).map(|agent| agent.current_loop_state())
  }

  /// Get agent count
  pub fn agent_count(&self) -> usize { self.agents.len() }

  /// Get list of all agent IDs
  pub fn agent_ids(&self) -> Vec<C::Address> { self.agents.keys().copied().collect() }
}

impl<C: Connection> Fabric<C> {
  /// Execute a single fabric step: poll transport and process messages
  pub fn start(&mut self) {
    for agent in self.agents.values_mut() {
      agent.signal_start();
    }
  }
}

impl<C: Connection> Default for Fabric<C> {
  fn default() -> Self { Self::new() }
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

/// Type alias for in-memory fabric
pub type InMemoryFabric = Fabric<InMemory>;

#[cfg(test)]
mod tests {
  use super::*;
  use crate::fixtures::*;

  pub struct Starter;

  impl LifeCycle for Starter {
    type StartMessage = TextMessage;
    type StopMessage = ();

    fn on_start(&mut self) -> Self::StartMessage {
      TextMessage { content: "Hello, world!".to_string() }
    }

    fn on_stop(&mut self) -> Self::StopMessage {}
  }

  #[test]
  fn test_fabric_basics() {
    let mut fabric = InMemoryFabric::new();

    // Register agents
    let counter_id = fabric.register_agent(Agent::new(Counter { total: 0 }));
    let logger_id = fabric
      .register_named_agent(
        "TestLogger",
        Agent::new(Logger { name: "TestLogger".to_string(), message_count: 0 }),
      )
      .unwrap();

    assert_eq!(fabric.agent_count(), 2);
    assert_eq!(fabric.agent_ids().len(), 2);
    assert!(fabric.agent_ids().contains(&counter_id));
    assert!(fabric.agent_ids().contains(&logger_id));

    assert_eq!(fabric.agent_id_by_name("TestLogger").unwrap(), logger_id);
  }

  #[test]
  fn test_fabric_example() {
    let mut fabric = InMemoryFabric::new();
    let starter_id = fabric.register_agent(Agent::new(Starter));
    let logger_id = fabric
      .register_named_agent(
        "TestLogger",
        Agent::new(Logger { name: "TestLogger".to_string(), message_count: 0 })
          .with_handler::<TextMessage>(),
      )
      .unwrap();

    fabric.start();
    let logger = fabric.agents.get(&logger_id).unwrap();
    let logger = logger.inner_as_any().downcast_ref::<Logger>().unwrap();
    assert_eq!(logger.message_count, 1);
  }
}
