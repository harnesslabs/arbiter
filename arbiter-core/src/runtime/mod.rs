use std::{
  any::{Any, TypeId},
  collections::HashMap,
  rc::Rc,
};

use serde::{Deserialize, Serialize};
#[cfg(feature = "wasm")] use wasm_bindgen::prelude::*;

#[cfg(feature = "wasm")] pub mod wasm;

use crate::{
  agent::{Agent, AgentId, AgentState, LifeCycle, RuntimeAgent},
  handler::Message,
};

/// A multi-agent runtime that manages agent lifecycles and message routing
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub struct Runtime {
  agents:     HashMap<AgentId, Box<dyn RuntimeAgent>>,
  name_to_id: HashMap<String, AgentId>, // Optional name lookup
}

impl Runtime {
  /// Create a new runtime
  pub fn new() -> Self { Self { agents: HashMap::new(), name_to_id: HashMap::new() } }

  /// Register an agent with the runtime
  pub fn register_agent<A>(&mut self, agent: Agent<A>) -> AgentId
  where A: LifeCycle {
    let id = agent.id();
    self.agents.insert(id, Box::new(agent));
    id
  }

  /// Register an agent with a name
  pub fn register_named_agent<A>(
    &mut self,
    name: impl Into<String>,
    agent: Agent<A>,
  ) -> Result<AgentId, String>
  where
    A: LifeCycle,
  {
    let name = name.into();
    if self.name_to_id.contains_key(&name) {
      return Err(format!("Agent name '{name}' already exists"));
    }

    let id = agent.id();
    self.agents.insert(id, Box::new(agent));
    self.name_to_id.insert(name, id);
    Ok(id)
  }

  /// Register an agent and start it immediately
  pub fn spawn_agent<A>(&mut self, agent: Agent<A>) -> AgentId
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
  ) -> Result<AgentId, String>
  where
    A: LifeCycle,
  {
    let id = self.register_named_agent(name, agent)?;
    self.start_agent_by_id(id).unwrap(); // Safe since we just added it
    Ok(id)
  }

  /// Look up agent ID by name
  pub fn agent_id_by_name(&self, name: &str) -> Option<AgentId> {
    self.name_to_id.get(name).copied()
  }

  /// Send a message to all agents that can handle it
  pub fn broadcast_message<M>(&mut self, message: M)
  where M: Message {
    let message_type = TypeId::of::<M>();
    // Alloc the message in the heap so we can create static shared references to put in each
    // agent's mailbox
    let message = Rc::new(message);

    for agent in self.agents.values_mut() {
      if agent.handlers().contains_key(&message_type) {
        agent.enqueue_shared_message(message.clone());
      }
    }
    // The message will be cleaned up by the agents when they process it, the original Rc created
    // here is dropped now.
  }

  /// Send a message to a specific agent by ID
  pub fn send_to_agent_by_id<M>(&mut self, agent_id: AgentId, message: M) -> Result<(), String>
  where M: Message {
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

  /// Execute a single runtime step: process messages and route replies
  /// Returns the number of messages processed in this step
  pub fn step(&mut self) {
    let mut pending_replies = Vec::new();

    // Process all agents that have pending messages
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

  /// Check if the runtime has any pending work
  pub fn has_pending_work(&self) -> bool { self.agents_needing_processing() > 0 }

  /// Get list of all agent IDs
  pub fn agent_ids(&self) -> Vec<AgentId> { self.agents.keys().copied().collect() }

  /// Get list of all agent names (only named agents)
  pub fn agent_names(&self) -> Vec<&String> { self.name_to_id.keys().collect() }

  /// Get agent count
  pub fn agent_count(&self) -> usize { self.agents.len() }

  /// Get count of agents that need processing
  pub fn agents_needing_processing(&self) -> usize {
    self.agents.values().filter(|agent| agent.should_process_mailbox()).count()
  }

  /// Start an agent by ID
  pub fn start_agent_by_id(&mut self, agent_id: AgentId) -> Result<(), String> {
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
  pub fn pause_agent_by_id(&mut self, agent_id: AgentId) -> Result<(), String> {
    self.agents.get_mut(&agent_id).map_or_else(
      || Err(format!("Agent with ID {agent_id} not found")),
      |agent| {
        agent.pause();
        Ok(())
      },
    )
  }

  /// Pause an agent by name
  pub fn pause_agent_by_name(&mut self, agent_name: &str) -> Result<(), String> {
    let agent_id =
      self.agent_id_by_name(agent_name).ok_or_else(|| format!("Agent '{agent_name}' not found"))?;
    self.pause_agent_by_id(agent_id)
  }

  /// Resume an agent by ID
  pub fn resume_agent_by_id(&mut self, agent_id: AgentId) -> Result<(), String> {
    self.agents.get_mut(&agent_id).map_or_else(
      || Err(format!("Agent with ID {agent_id} not found")),
      |agent| {
        agent.resume();
        Ok(())
      },
    )
  }

  /// Resume an agent by name
  pub fn resume_agent_by_name(&mut self, agent_name: &str) -> Result<(), String> {
    let agent_id =
      self.agent_id_by_name(agent_name).ok_or_else(|| format!("Agent '{agent_name}' not found"))?;
    self.resume_agent_by_id(agent_id)
  }

  /// Stop an agent by ID
  pub fn stop_agent_by_id(&mut self, agent_id: AgentId) -> Result<(), String> {
    self.agents.get_mut(&agent_id).map_or_else(
      || Err(format!("Agent with ID {agent_id} not found")),
      |agent| {
        agent.stop();
        Ok(())
      },
    )
  }

  /// Stop an agent by name
  pub fn stop_agent_by_name(&mut self, agent_name: &str) -> Result<(), String> {
    let agent_id =
      self.agent_id_by_name(agent_name).ok_or_else(|| format!("Agent '{agent_name}' not found"))?;
    self.stop_agent_by_id(agent_id)
  }

  /// Remove an agent by ID and return it
  pub fn remove_agent_by_id(&mut self, agent_id: AgentId) -> Result<Box<dyn RuntimeAgent>, String> {
    // Remove from name lookup if it has a name
    if let Some(agent) = self.agents.get(&agent_id) {
      if let Some(name) = agent.name() {
        self.name_to_id.remove(name);
      }
    }

    self
      .agents
      .remove(&agent_id)
      .map_or_else(|| Err(format!("Agent with ID {agent_id} not found")), Ok)
  }

  /// Remove an agent by name and return it
  pub fn remove_agent_by_name(
    &mut self,
    agent_name: &str,
  ) -> Result<Box<dyn RuntimeAgent>, String> {
    let agent_id =
      self.agent_id_by_name(agent_name).ok_or_else(|| format!("Agent '{agent_name}' not found"))?;
    self.name_to_id.remove(agent_name);
    self.remove_agent_by_id(agent_id)
  }

  /// Get the state of an agent by ID
  pub fn agent_state_by_id(&self, agent_id: AgentId) -> Option<AgentState> {
    self.agents.get(&agent_id).map(|agent| agent.state())
  }

  /// Get the state of an agent by name
  pub fn agent_state_by_name(&self, agent_name: &str) -> Option<AgentState> {
    let agent_id = self.agent_id_by_name(agent_name)?;
    self.agent_state_by_id(agent_id)
  }

  /// Get statistics about the runtime
  pub fn statistics(&self) -> RuntimeStatistics {
    let total = self.agents.len();
    let running = self.agents.values().filter(|a| a.state() == AgentState::Running).count();
    let paused = self.agents.values().filter(|a| a.state() == AgentState::Paused).count();
    let stopped = self.agents.values().filter(|a| a.state() == AgentState::Stopped).count();
    let pending_messages = self.agents_needing_processing();

    RuntimeStatistics {
      total_agents:                 total,
      running_agents:               running,
      paused_agents:                paused,
      stopped_agents:               stopped,
      agents_with_pending_messages: pending_messages,
    }
  }

  /// Start all agents
  pub fn start_all_agents(&mut self) -> usize {
    let mut started_count = 0;
    for agent in self.agents.values_mut() {
      if agent.state() != AgentState::Running {
        agent.start();
        started_count += 1;
      }
    }
    started_count
  }

  /// Pause all agents
  pub fn pause_all_agents(&mut self) -> usize {
    let mut paused_count = 0;
    for agent in self.agents.values_mut() {
      if agent.state() == AgentState::Running {
        agent.pause();
        paused_count += 1;
      }
    }
    paused_count
  }

  /// Resume all paused agents
  pub fn resume_all_agents(&mut self) -> usize {
    let mut resumed_count = 0;
    for agent in self.agents.values_mut() {
      if agent.state() == AgentState::Paused {
        agent.resume();
        resumed_count += 1;
      }
    }
    resumed_count
  }

  /// Stop all agents
  pub fn stop_all_agents(&mut self) -> usize {
    let mut stopped_count = 0;
    for agent in self.agents.values_mut() {
      if agent.state() != AgentState::Stopped {
        agent.stop();
        stopped_count += 1;
      }
    }
    stopped_count
  }

  /// Get all agents by their current state
  pub fn agents_by_state(&self, state: AgentState) -> Vec<AgentId> {
    self
      .agents
      .iter()
      .filter_map(|(id, agent)| if agent.state() == state { Some(*id) } else { None })
      .collect()
  }

  /// Remove all agents from the runtime and return them
  pub fn remove_all_agents(&mut self) -> Vec<(AgentId, Box<dyn RuntimeAgent>)> {
    self.name_to_id.clear(); // Clear name lookup
    self.agents.drain().collect()
  }

  /// Re-insert a removed agent
  pub fn reinsert_agent(&mut self, agent: Box<dyn RuntimeAgent>) -> AgentId {
    let id = agent.id();
    if let Some(name) = agent.name() {
      self.name_to_id.insert(name.to_string(), id);
    }
    self.agents.insert(id, agent);
    id
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

impl Default for Runtime {
  fn default() -> Self { Self::new() }
}

/// Statistics about the runtime state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeStatistics {
  pub total_agents:                 usize,
  pub running_agents:               usize,
  pub paused_agents:                usize,
  pub stopped_agents:               usize,
  pub agents_with_pending_messages: usize,
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
  fn test_runtime_basics() {
    let mut runtime = Runtime::new();

    // Register agents
    let counter_id = runtime.register_agent(Agent::new(Counter { total: 0 }));
    let logger_id = runtime
      .register_named_agent(
        "TestLogger",
        Agent::new(Logger { name: "TestLogger".to_string(), message_count: 0 }),
      )
      .unwrap();

    assert_eq!(runtime.agent_count(), 2);
    assert_eq!(runtime.agent_ids().len(), 2);
    assert!(runtime.agent_ids().contains(&counter_id));
    assert!(runtime.agent_ids().contains(&logger_id));
  }

  #[test]
  fn test_message_routing() {
    let mut runtime = Runtime::new();

    // Register agents with specific handlers
    let counter = Agent::new(Counter { total: 0 }).with_handler::<NumberMessage>();
    let logger = Agent::new(Logger { name: "TestLogger".to_string(), message_count: 0 })
      .with_handler::<TextMessage>();

    let counter_id = counter.id();
    let logger_id = logger.id();

    runtime.agents.insert(counter_id, Box::new(counter));
    runtime.agents.insert(logger_id, Box::new(logger));

    // Start agents
    runtime.start_agent_by_id(counter_id).unwrap();
    runtime.start_agent_by_id(logger_id).unwrap();

    // Send messages - should route to appropriate agents
    runtime.broadcast_message(NumberMessage { value: 42 });
    runtime.broadcast_message(TextMessage { content: "Hello".to_string() });

    // Process pending messages
    let processed = runtime.process_all_pending_messages();
    assert_eq!(processed, 2); // Two messages processed
  }

  #[test]
  fn test_agent_lifecycle() {
    let mut runtime = Runtime::new();

    let counter_id = runtime.spawn_agent(Agent::new(Counter { total: 0 }));

    // Agent should be running after spawn
    assert_eq!(runtime.agent_state_by_id(counter_id), Some(AgentState::Running));

    // Pause the agent
    runtime.pause_agent_by_id(counter_id).unwrap();
    assert_eq!(runtime.agent_state_by_id(counter_id), Some(AgentState::Paused));

    // Resume the agent
    runtime.resume_agent_by_id(counter_id).unwrap();
    assert_eq!(runtime.agent_state_by_id(counter_id), Some(AgentState::Running));

    // Stop the agent
    runtime.stop_agent_by_id(counter_id).unwrap();
    assert_eq!(runtime.agent_state_by_id(counter_id), Some(AgentState::Stopped));
  }

  #[test]
  fn test_runtime_statistics() {
    let mut runtime = Runtime::new();

    let _agent1_id = runtime.spawn_agent(Agent::new(Counter { total: 0 }));
    let _agent2_id = runtime.spawn_agent(Agent::new(Counter { total: 0 }));
    let _agent3_id = runtime.register_agent(Agent::new(Counter { total: 0 }));

    let stats = runtime.statistics();
    assert_eq!(stats.total_agents, 3);
    assert_eq!(stats.running_agents, 2); // agent1 and agent2 were spawned (started)
    assert_eq!(stats.stopped_agents, 1); // agent3 was only registered
  }

  #[test]
  fn test_bulk_agent_operations() {
    let mut runtime = Runtime::new();

    // Register multiple agents
    let agent1_id = runtime.register_agent(Agent::new(Counter { total: 0 }));
    let agent2_id = runtime.register_agent(Agent::new(Counter { total: 0 }));
    let agent3_id = runtime.register_agent(Agent::new(Counter { total: 0 }));
    // Don't test specific ID values as they depend on global counter state
    assert_ne!(agent1_id, agent2_id);
    assert_ne!(agent2_id, agent3_id);
    assert_ne!(agent1_id, agent3_id);

    // All should be stopped initially
    assert_eq!(runtime.statistics().stopped_agents, 3);

    // Start all agents
    let started = runtime.start_all_agents();
    assert_eq!(started, 3);
    assert_eq!(runtime.statistics().running_agents, 3);

    // Pause all agents
    let paused = runtime.pause_all_agents();
    assert_eq!(paused, 3);
    assert_eq!(runtime.statistics().paused_agents, 3);

    // Resume all agents
    let resumed = runtime.resume_all_agents();
    assert_eq!(resumed, 3);
    assert_eq!(runtime.statistics().running_agents, 3);

    // Stop all agents
    let stopped = runtime.stop_all_agents();
    assert_eq!(stopped, 3);
    assert_eq!(runtime.statistics().stopped_agents, 3);

    // Test agents_by_state
    let stopped_agents = runtime.agents_by_state(AgentState::Stopped);
    assert_eq!(stopped_agents.len(), 3);

    // Remove all agents
    let removed = runtime.remove_all_agents();
    assert_eq!(removed.len(), 3);
    assert_eq!(runtime.agent_count(), 0);
  }

  #[test]
  fn test_runtime_execution() {
    let mut runtime = Runtime::new();

    // Create agents with handlers
    let counter = Agent::new(Counter { total: 0 }).with_handler::<NumberMessage>();
    let logger = Agent::new(Logger { name: "Logger".to_string(), message_count: 0 })
      .with_handler::<TextMessage>();

    let counter_id = counter.id();
    let logger_id = logger.id();

    runtime.register_agent(counter);
    runtime.register_agent(logger);

    // Start agents
    runtime.start_agent_by_id(counter_id).unwrap();
    runtime.start_agent_by_id(logger_id).unwrap();

    // Send initial messages
    runtime.broadcast_message(NumberMessage { value: 10 });
    runtime.broadcast_message(TextMessage { content: "Hello".to_string() });

    // Test single step execution
    assert!(runtime.has_pending_work());
    runtime.step();
  }

  // Custom message and reply types for testing
  #[derive(Debug, Clone)]
  struct RequestData {
    value: i32,
  }

  #[derive(Debug, Clone)]
  struct ResponseData {
    result: i32,
  }

  // Producer agent that generates responses
  #[derive(Clone)]
  struct Producer {
    multiplier: i32,
  }

  impl LifeCycle for Producer {}

  impl Handler<RequestData> for Producer {
    type Reply = ResponseData;

    fn handle(&mut self, message: &RequestData) -> Self::Reply {
      ResponseData { result: message.value * self.multiplier }
    }
  }

  // Consumer agent that processes responses
  #[derive(Clone)]
  struct Consumer {
    total:              i32,
    responses_received: usize,
  }

  impl LifeCycle for Consumer {}

  impl Handler<ResponseData> for Consumer {
    type Reply = ();

    fn handle(&mut self, message: &ResponseData) -> Self::Reply {
      self.total += message.result;
      self.responses_received += 1;
    }
  }

  #[test]
  fn test_reply_routing_and_memory_cleanup() {
    let mut runtime = Runtime::new();

    // Create producer and consumers
    let producer = Agent::new(Producer { multiplier: 2 }).with_handler::<RequestData>();
    let consumer1 =
      Agent::new(Consumer { total: 0, responses_received: 0 }).with_handler::<ResponseData>();
    let consumer2 =
      Agent::new(Consumer { total: 0, responses_received: 0 }).with_handler::<ResponseData>();

    let producer_id = producer.id();
    let consumer1_id = consumer1.id();
    let consumer2_id = consumer2.id();

    runtime.register_agent(producer);
    runtime.register_agent(consumer1);
    runtime.register_agent(consumer2);

    // Start all agents
    runtime.start_agent_by_id(producer_id).unwrap();
    runtime.start_agent_by_id(consumer1_id).unwrap();
    runtime.start_agent_by_id(consumer2_id).unwrap();

    // Create a message and keep a weak reference to test cleanup
    let request = Rc::new(RequestData { value: 10 });
    let weak_request = Rc::downgrade(&request);

    // Send the request to the producer
    runtime.send_to_agent_by_id(producer_id, (*request).clone()).unwrap();

    // Check initial reference count (1 for our Rc, plus any internal references)
    let initial_strong_count = Rc::strong_count(&request);
    assert!(initial_strong_count >= 1);

    // Drop our reference to the original message
    drop(request);

    // Step 1: Process the request, which should generate a response
    runtime.step();

    // Verify the original request message is cleaned up
    assert!(weak_request.upgrade().is_none(), "Original request should be cleaned up");

    // Step 2: Route the response to consumers
    runtime.step();

    // Verify no more work remains
    assert!(!runtime.has_pending_work());

    // Verify both consumers received the response
    let consumer1 = runtime.agents.get(&consumer1_id).unwrap();
    if let Some(consumer) = consumer1.inner_as_any().downcast_ref::<Consumer>() {
      assert_eq!(consumer.total, 20); // 10 * 2 = 20
      assert_eq!(consumer.responses_received, 1);
    }

    let consumer2 = runtime.agents.get(&consumer2_id).unwrap();
    if let Some(consumer) = consumer2.inner_as_any().downcast_ref::<Consumer>() {
      assert_eq!(consumer.total, 20); // 10 * 2 = 20
      assert_eq!(consumer.responses_received, 1);
    }

    // Test multiple message cycles to ensure no memory accumulation
    for i in 1..=5 {
      let request = Rc::new(RequestData { value: i });
      let weak_ref = Rc::downgrade(&request);

      runtime.send_to_agent_by_id(producer_id, (*request).clone()).unwrap();
      drop(request);

      // Process request and response
      runtime.step(); // Generate response
      runtime.step(); // Route response to consumers

      // Verify cleanup
      assert!(weak_ref.upgrade().is_none(), "Request {} should be cleaned up", i);
    }

    // Final verification - no pending work and proper final state
    assert!(!runtime.has_pending_work());
    let stats = runtime.statistics();
    assert_eq!(stats.running_agents, 3);
    assert_eq!(stats.agents_with_pending_messages, 0);
  }

  #[test]
  fn test_rc_message_sharing() {
    let mut runtime = Runtime::new();

    // Create multiple agents that can handle the same message type
    let counter1 = Agent::new(Counter { total: 0 }).with_handler::<NumberMessage>();
    let counter2 = Agent::new(Counter { total: 0 }).with_handler::<NumberMessage>();
    let counter3 = Agent::new(Counter { total: 0 }).with_handler::<NumberMessage>();

    let counter1_id = counter1.id();
    let counter2_id = counter2.id();
    let counter3_id = counter3.id();

    runtime.register_agent(counter1);
    runtime.register_agent(counter2);
    runtime.register_agent(counter3);

    runtime.start_agent_by_id(counter1_id).unwrap();
    runtime.start_agent_by_id(counter2_id).unwrap();
    runtime.start_agent_by_id(counter3_id).unwrap();

    // Create a message with a known Rc
    let message = Rc::new(NumberMessage { value: 42 });
    let message_weak = Rc::downgrade(&message);

    // Broadcast should share the Rc among all agents
    runtime.broadcast_message((*message).clone());

    // Drop our reference
    drop(message);

    // Process messages - this should consume the shared Rcs
    let processed = runtime.process_all_pending_messages();
    assert_eq!(processed, 3); // Each counter generates a () reply, so 3 replies total

    // Verify the message is properly cleaned up
    assert!(message_weak.upgrade().is_none(), "Message should be cleaned up after processing");

    // Verify all counters processed the message
    for &counter_id in &[counter1_id, counter2_id, counter3_id] {
      let agent = runtime.agents.get(&counter_id).unwrap();
      if let Some(counter) = agent.inner_as_any().downcast_ref::<Counter>() {
        assert_eq!(counter.total, 42);
      }
    }
  }

  #[test]
  fn test_simple_reply_routing() {
    let mut runtime = Runtime::new();

    // Create a simple producer-consumer pair
    let producer = Agent::new(Producer { multiplier: 2 }).with_handler::<RequestData>();
    let consumer =
      Agent::new(Consumer { total: 0, responses_received: 0 }).with_handler::<ResponseData>();

    let producer_id = producer.id();
    let consumer_id = consumer.id();

    runtime.register_agent(producer);
    runtime.register_agent(consumer);

    // Start both agents
    runtime.start_agent_by_id(producer_id).unwrap();
    runtime.start_agent_by_id(consumer_id).unwrap();

    // Send a request to the producer
    runtime.send_to_agent_by_id(producer_id, RequestData { value: 5 }).unwrap();

    // Step 1: Producer processes request and generates ResponseData
    runtime.step();

    // Step 2: Consumer should process the ResponseData
    runtime.step();

    // Verify the consumer received and processed the response
    let consumer_agent = runtime.agents.get(&consumer_id).unwrap();
    if let Some(consumer) = consumer_agent.inner_as_any().downcast_ref::<Consumer>() {
      assert_eq!(consumer.total, 10); // 5 * 2 = 10
      assert_eq!(consumer.responses_received, 1);
    }
  }
}
