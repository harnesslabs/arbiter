use std::{
  any::{Any, TypeId},
  collections::HashMap,
  rc::Rc,
};

use crate::{
  agent::{Agent, AgentState, LifeCycle, RuntimeAgent},
  handler::Message,
  transport::{
    memory::{InMemoryEnvelope, InMemoryTransport},
    AgentIdentity, Envelope, FabricId, Transport, TransportLayer,
  },
};

/// A generic fabric that manages agents over a specific transport layer
pub struct Fabric<T: TransportLayer> {
  id:         FabricId,
  transport:  T,
  agents:     HashMap<AgentIdentity, Box<dyn RuntimeAgent>>,
  name_to_id: HashMap<String, AgentIdentity>,
}

impl<T: TransportLayer> Fabric<T> {
  /// Create a new fabric with the given transport
  pub fn new(transport: T) -> Self {
    Self { id: FabricId::generate(), transport, agents: HashMap::new(), name_to_id: HashMap::new() }
  }

  /// Get the fabric's unique identifier
  pub fn id(&self) -> FabricId { self.id }

  /// Register an agent with the fabric
  pub fn register_agent<A>(&mut self, agent: Agent<A>) -> AgentIdentity
  where A: LifeCycle {
    let id = AgentIdentity::generate(); // Generate new cross-fabric identity
    self.agents.insert(id, Box::new(agent));
    id
  }

  /// Register an agent with a name
  pub fn register_named_agent<A>(
    &mut self,
    name: impl Into<String>,
    agent: Agent<A>,
  ) -> Result<AgentIdentity, String>
  where
    A: LifeCycle,
  {
    let name = name.into();
    if self.name_to_id.contains_key(&name) {
      return Err(format!("Agent name '{name}' already exists"));
    }

    let id = AgentIdentity::generate();
    self.agents.insert(id, Box::new(agent));
    self.name_to_id.insert(name, id);
    Ok(id)
  }

  /// Register an agent and start it immediately
  pub fn spawn_agent<A>(&mut self, agent: Agent<A>) -> AgentIdentity
  where A: LifeCycle {
    let id = self.register_agent(agent);
    self.start_agent_by_id(id).unwrap(); // Safe since we just added it
    id
  }

  /// Register a named agent and start it immediately
  pub fn spawn_named_agent<A>(
    &mut self,
    name: impl Into<String>,
    agent: Agent<A>,
  ) -> Result<AgentIdentity, String>
  where
    A: LifeCycle,
  {
    let id = self.register_named_agent(name, agent)?;
    self.start_agent_by_id(id).unwrap(); // Safe since we just added it
    Ok(id)
  }

  /// Look up agent ID by name
  pub fn agent_id_by_name(&self, name: &str) -> Option<AgentIdentity> {
    self.name_to_id.get(name).copied()
  }

  /// Start an agent by ID
  pub fn start_agent_by_id(&mut self, agent_id: AgentIdentity) -> Result<(), String> {
    self.agents.get_mut(&agent_id).map_or_else(
      || Err(format!("Agent with ID {agent_id} not found")),
      |agent| {
        agent.start();
        Ok(())
      },
    )
  }

  /// Start an agent by name
  pub fn start_agent_by_name(&mut self, agent_name: &str) -> Result<(), String> {
    let agent_id =
      self.agent_id_by_name(agent_name).ok_or_else(|| format!("Agent '{agent_name}' not found"))?;
    self.start_agent_by_id(agent_id)
  }

  /// Pause an agent by ID
  pub fn pause_agent_by_id(&mut self, agent_id: AgentIdentity) -> Result<(), String> {
    self.agents.get_mut(&agent_id).map_or_else(
      || Err(format!("Agent with ID {agent_id} not found")),
      |agent| {
        agent.pause();
        Ok(())
      },
    )
  }

  /// Get the state of an agent by ID
  pub fn agent_state_by_id(&self, agent_id: AgentIdentity) -> Option<AgentState> {
    self.agents.get(&agent_id).map(|agent| agent.state())
  }

  /// Get agent count
  pub fn agent_count(&self) -> usize { self.agents.len() }

  /// Get list of all agent IDs
  pub fn agent_ids(&self) -> Vec<AgentIdentity> { self.agents.keys().copied().collect() }

  /// Check if the fabric has any pending work
  pub fn has_pending_work(&self) -> bool { self.agents_needing_processing() > 0 }

  /// Get count of agents that need processing
  pub fn agents_needing_processing(&self) -> usize {
    self.agents.values().filter(|agent| agent.should_process_mailbox()).count()
  }

  /// Execute a single fabric step: poll transport and process messages
  pub fn step(&mut self) {
    // Poll transport for incoming envelopes
    let envelopes = self.transport.poll();

    // Process each envelope
    for envelope in envelopes {
      // This needs to be implemented per transport type
      // For now, InMemoryTransport has a specialized implementation
    }

    // Process agent mailboxes and collect replies
    let mut pending_replies = Vec::new();
    for agent in self.agents.values_mut() {
      if agent.should_process_mailbox() {
        let replies = agent.process_pending_messages();
        pending_replies.extend(replies);
      }
    }

    // Route all replies to appropriate agents
    for reply in pending_replies {
      self.route_reply_to_agents(reply);
    }
  }

  /// Helper function to route reply messages back into the system
  fn route_reply_to_agents(&mut self, reply: Rc<dyn Any>) {
    let reply_type = reply.as_ref().type_id();

    for agent in self.agents.values_mut() {
      if agent.handlers().contains_key(&reply_type) {
        agent.enqueue_shared_message(reply.clone());
      }
    }
  }
}

impl<T> Default for Fabric<T>
where T: TransportLayer + Default
{
  fn default() -> Self { Self::new(T::default()) }
}

// Specialized implementation for InMemoryTransport that preserves current Runtime semantics
impl Fabric<InMemoryTransport> {
  /// Send a message to all agents that can handle it (broadcast)
  pub fn broadcast_message<M>(&mut self, message: M)
  where M: Message {
    let message_type = TypeId::of::<M>();
    let message = Rc::new(message);

    for agent in self.agents.values_mut() {
      if agent.handlers().contains_key(&message_type) {
        agent.enqueue_shared_message(message.clone());
      }
    }
  }

  /// Send a message to a specific agent by ID
  pub fn send_to_agent_by_id<M>(
    &mut self,
    agent_id: AgentIdentity,
    message: M,
  ) -> Result<(), String>
  where
    M: Message,
  {
    let message = Rc::new(message);
    self.agents.get_mut(&agent_id).map_or_else(
      || Err(format!("Agent with ID {agent_id} not found")),
      |agent| {
        agent.enqueue_shared_message(message.clone());
        Ok(())
      },
    )
  }

  /// Send a message to a specific agent by name
  pub fn send_to_agent_by_name<M>(&mut self, agent_name: &str, message: M) -> Result<(), String>
  where M: Message {
    let agent_id =
      self.agent_id_by_name(agent_name).ok_or_else(|| format!("Agent '{agent_name}' not found"))?;
    self.send_to_agent_by_id(agent_id, message)
  }

  /// Process all pending messages across all agents
  pub fn process_all_pending_messages(&mut self) -> usize {
    let mut total_processed = 0;
    let mut all_replies = Vec::new();

    // First pass: collect all replies without routing them
    for agent in self.agents.values_mut() {
      if agent.should_process_mailbox() {
        let replies = agent.process_pending_messages();
        total_processed += replies.len();
        all_replies.extend(replies);
      }
    }

    // Second pass: route all collected replies back into agent mailboxes
    for reply in all_replies {
      self.route_reply_to_agents(reply);
    }

    total_processed
  }

  /// Override process_envelope for in-memory transport
  fn process_envelope(&mut self, envelope: InMemoryEnvelope) {
    // For in-memory, we can directly use the Rc<dyn Any> message
    let message = envelope.message().clone();
    let message_type = envelope.message_type();

    // Route to specific agent if addressed to one
    if let Some(agent) = self.agents.get_mut(envelope.address_to()) {
      if agent.handlers().contains_key(&message_type) {
        agent.enqueue_shared_message(message);
      }
    }
  }
}

/// Type alias for backward compatibility
pub type InMemoryFabric = Fabric<InMemoryTransport>;

impl Default for InMemoryTransport {
  fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
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
    let mut fabric = InMemoryFabric::new(InMemoryTransport::new());

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
    let mut fabric = InMemoryFabric::new(InMemoryTransport::new());

    // Register agents with specific handlers
    let counter = Agent::new(Counter { total: 0 }).with_handler::<NumberMessage>();
    let logger = Agent::new(Logger { name: "TestLogger".to_string(), message_count: 0 })
      .with_handler::<TextMessage>();

    let counter_id = fabric.register_agent(counter);
    let logger_id = fabric.register_agent(logger);

    // Start agents
    fabric.start_agent_by_id(counter_id).unwrap();
    fabric.start_agent_by_id(logger_id).unwrap();

    // Send messages - should route to appropriate agents
    fabric.broadcast_message(NumberMessage { value: 42 });
    fabric.broadcast_message(TextMessage { content: "Hello".to_string() });

    // Process pending messages
    let processed = fabric.process_all_pending_messages();
    assert_eq!(processed, 2); // Two messages processed
  }
}
