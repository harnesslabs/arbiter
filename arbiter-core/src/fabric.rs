use std::{any::Any, collections::HashMap, rc::Rc};

use crate::{
  agent::{Agent, AgentState, LifeCycle, RuntimeAgent},
  transport::{
    memory::InMemoryTransport, AgentIdentity, Envelope, FabricId, SendTarget, Transport,
  },
};

/// A generic fabric that manages agents over a specific transport layer
pub struct Fabric<T: Transport> {
  id:         FabricId,
  transport:  T,
  agents:     HashMap<AgentIdentity, Box<dyn RuntimeAgent<T>>>,
  name_to_id: HashMap<String, AgentIdentity>,
}

impl<T: Transport> Fabric<T> {
  /// Create a new fabric with the given transport
  pub fn new() -> Self {
    Self {
      id:         FabricId::generate(),
      transport:  T::new(),
      agents:     HashMap::new(),
      name_to_id: HashMap::new(),
    }
  }

  /// Get the fabric's unique identifier
  pub fn id(&self) -> FabricId { self.id }

  /// Register an agent with the fabric
  pub fn register_agent<A>(&mut self, agent: Agent<A, T>) -> AgentIdentity
  where A: LifeCycle {
    let id = AgentIdentity::generate();
    self.agents.insert(id, Box::new(agent));
    id
  }

  /// Register an agent with a name
  pub fn register_named_agent<A>(
    &mut self,
    name: impl Into<String>,
    agent: Agent<A, T>,
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
  pub fn spawn_agent<A>(&mut self, agent: Agent<A, T>) -> AgentIdentity
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
    // Poll transport for incoming envelopes (these came from the `send` method)
    let envelopes = self.transport.poll();

    // Process each envelope and send
    for envelope in envelopes {
      for agent in self.agents.values_mut() {
        // TODO: WE clone here, but really should do some shared reference or something. This is not
        // necessarily a light weight clone at all. We need some way of encapsulating what kind of
        // sharing we are doing here.
        // TODO: This also greedily gives the agent the message which it may not handle, but that is
        // caught in the `process_pending_messages` method (this is okay)
        if envelope.to == SendTarget::Address(agent.address())
          || envelope.to == SendTarget::Broadcast
        {
          agent.queue_message(envelope.payload.clone());
        }
      }
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
  fn route_reply_to_agents(&mut self, reply: T::Payload) {
    let reply_type = reply.type_id();

    for agent in self.agents.values_mut() {
      if agent.handlers().contains_key(&reply_type) {
        // TODO: We clone here, where this may be a lightweight clone like Rc<dyn Message>
        agent.queue_message(reply.clone());
      }
    }
  }

  pub fn broadcast_message(&mut self, message: T::Payload) {
    for agent in self.agents.values_mut() {
      agent.queue_message(message.clone());
    }
  }
}

impl<T> Default for Fabric<T>
where T: Transport + Default
{
  fn default() -> Self { Self::new() }
}

/// Type alias for in-memory fabric
pub type InMemoryFabric = Fabric<InMemoryTransport>;

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

    // TODO: Implement broadcast and send methods for generic fabric
    fabric.broadcast_message(Rc::new(NumberMessage { value: 42 }));
    fabric.broadcast_message(Rc::new(TextMessage { content: "Hello".to_string() }));

    // Process pending messages
    fabric.step();
    let counter =
      fabric.agents.get(&counter_id).unwrap().inner_as_any().downcast_ref::<Counter>().unwrap();
    let logger =
      fabric.agents.get(&logger_id).unwrap().inner_as_any().downcast_ref::<Logger>().unwrap();

    assert_eq!(counter.total, 42);
    assert_eq!(logger.message_count, 1);
  }
}
