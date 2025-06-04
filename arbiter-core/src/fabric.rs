use std::{collections::HashMap, sync::Arc, thread::JoinHandle};

use crate::{
  agent::{Agent, AgentState, CommunicationChannel, LifeCycle, RuntimeAgent, State},
  transport::{memory::InMemoryTransport, Runtime, SyncRuntime, Transport},
};

/// A generic fabric that manages agents over a specific transport layer
// TODO: Instead of storing agents, we could just store the transport components (e.g., a sender
// and an atomic for their state)
pub struct Fabric<T: Transport<Runtime = R>, R: Runtime> {
  id:        FabricId,
  transport: T,

  // TODO: This should store a task handle for each RUNNING or PAUSEED agent, we should dump
  // stopped agents into a different list.
  agents:     HashMap<T::Address, Box<dyn RuntimeAgent<T>>>,
  name_to_id: HashMap<String, T::Address>,
  channels:   HashMap<T::Address, CommunicationChannel<T>>,
}

impl<T: Transport<Runtime = R>, R: Runtime> Fabric<T, R>
where T::Payload: std::fmt::Debug
{
  /// Create a new fabric with the given transport
  pub fn new() -> Self {
    Self {
      id:         FabricId::generate(),
      transport:  T::new(),
      agents:     HashMap::new(),
      name_to_id: HashMap::new(),
      channels:   HashMap::new(),
    }
  }

  /// Get the fabric's unique identifier
  pub fn id(&self) -> FabricId { self.id }

  /// Register an agent with the fabric
  pub fn register_agent<A>(&mut self, agent: Agent<A, T>) -> T::Address
  where A: LifeCycle {
    let tx = agent.communication_channel();
    let id = agent.address();
    self.channels.insert(id, tx);

    id
  }

  /// Register an agent with a name
  pub fn register_named_agent<A>(
    &mut self,
    name: impl Into<String>,
    agent: Agent<A, T>,
  ) -> Result<T::Address, String>
  where
    A: LifeCycle,
  {
    let name = name.into();
    if self.name_to_id.contains_key(&name) {
      return Err(format!("Agent name '{name}' already exists"));
    }
    let id = agent.address();
    self.name_to_id.insert(name, id);

    Ok(id)
  }

  /// Register an agent and start it immediately
  pub fn spawn_agent<A>(&mut self, agent: Agent<A, T>) -> T::Address
  where A: LifeCycle {
    let id = self.register_agent(agent);
    self.start_agent_by_id(id).unwrap(); // Safe since we just added it
    id
  }

  /// Register a named agent and start it immediately
  pub fn spawn_named_agent<A>(
    &mut self,
    name: impl Into<String>,
    agent: Agent<A, T>,
  ) -> Result<T::Address, String>
  where
    A: LifeCycle,
  {
    let id = self.register_named_agent(name, agent)?;
    self.start_agent_by_id(id).unwrap(); // Safe since we just added it
    Ok(id)
  }

  /// Look up agent ID by name
  pub fn agent_id_by_name(&self, name: &str) -> Option<T::Address> {
    self.name_to_id.get(name).copied()
  }

  /// Start an agent by ID
  pub fn start_agent_by_id(&mut self, agent_id: T::Address) -> Result<(), String> {
    self.channels.get_mut(&agent_id).map_or_else(
      || Err(format!("Agent with ID {agent_id} not found")),
      |sender| {
        sender.start();
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
  pub fn agent_state_by_id(&self, agent_id: T::Address) -> Option<AgentState> {
    self.agents.get(&agent_id).map(|agent| match agent {
      State::Running(_) => AgentState::Running,
      State::Stopped(_) => AgentState::Stopped,
    })
  }

  /// Get agent count
  pub fn agent_count(&self) -> usize { self.agents.len() }

  /// Get list of all agent IDs
  pub fn agent_ids(&self) -> Vec<T::Address> { self.agents.keys().copied().collect() }

  /// Helper function to broadcast messages to all agents
  fn broadcast(&mut self, payload: T::Payload) {
    for sender in self.channels.values_mut() {
      sender.send(payload.clone());
    }
  }
}

impl<T: Transport<Runtime = R>, R: Runtime> Fabric<T, R>
where T::Payload: std::fmt::Debug
{
  /// Execute a single fabric step: poll transport and process messages
  pub fn start(&mut self) {
    for sender in self.channels.values_mut() {
      sender.start();
    }
  }
}

impl<T: Transport<Runtime = R>, R: Runtime> Default for Fabric<T, R>
where T::Payload: std::fmt::Debug
{
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
pub type InMemoryFabric = Fabric<InMemoryTransport, SyncRuntime>;

#[cfg(test)]
mod tests {
  use std::rc::Rc;

  use super::*;
  use crate::handler::Handler;

  // Example message types - just regular structs!
  #[derive(Debug, Clone)]
  struct NumberMessage {
    value: i32,
  }

  #[derive(Debug, Clone)]
  struct TextMessage {
    content: String,
  }

  // Example agent types - simple structs with clear state
  #[derive(Clone)]
  struct Counter {
    total: i32,
  }

  impl LifeCycle for Counter {}

  #[derive(Clone)]
  struct Logger {
    name:          String,
    message_count: i32,
  }

  impl LifeCycle for Logger {}

  impl Handler<NumberMessage> for Counter {
    type Reply = ();

    fn handle(&mut self, message: &NumberMessage) -> Self::Reply {
      self.total += message.value;
      println!("CounterAgent total is now: {}", self.total);
    }
  }

  impl Handler<TextMessage> for Logger {
    type Reply = ();

    fn handle(&mut self, message: &TextMessage) -> Self::Reply {
      self.message_count += 1;
      println!(
        "LogAgent '{}' received: '{}' (count: {})",
        self.name, message.content, self.message_count
      );
    }
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
  }

  #[test]
  fn test_fabric_message_routing() {
    let mut fabric = InMemoryFabric::new();

    // Register agents with specific handlers
    let counter = Agent::new(Counter { total: 0 }).with_handler::<NumberMessage>();
    let logger = Agent::new(Logger { name: "TestLogger".to_string(), message_count: 0 })
      .with_handler::<TextMessage>();

    let counter_id = fabric.register_agent(counter);
    let logger_id = fabric.register_agent(logger);

    // Start agents
    fabric.start_agent_by_id(counter_id).unwrap();
    fabric.start_agent_by_id(logger_id).unwrap();

    fabric.broadcast(Rc::new(NumberMessage { value: 42 }));
    fabric.broadcast(Rc::new(TextMessage { content: "Hello".to_string() }));

    // Process pending messages
    fabric.start();
    let counter =
      fabric.agents.get(&counter_id).unwrap().inner_as_any().downcast_ref::<Counter>().unwrap();
    let logger =
      fabric.agents.get(&logger_id).unwrap().inner_as_any().downcast_ref::<Logger>().unwrap();

    assert_eq!(counter.total, 42);
    assert_eq!(logger.message_count, 1);
  }
}
