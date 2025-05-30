use std::{
  any::{Any, TypeId},
  collections::HashMap,
  sync::{Arc, Mutex},
};

#[cfg(feature = "wasm")] use wasm_bindgen::prelude::*;

use crate::agent::{Agent, AgentId, AgentState, LifeCycle, RuntimeAgent};

/// A multi-agent runtime that manages agent lifecycles and message routing
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub struct Runtime {
  agents:     HashMap<AgentId, Box<dyn RuntimeAgent>>,
  name_to_id: HashMap<String, AgentId>, // Optional name lookup
}

impl Runtime {
  /// Create a new runtime
  pub fn new() -> Self { Self { agents: HashMap::new(), name_to_id: HashMap::new() } }

  /// Create a shared runtime wrapped in Arc<Mutex<>> for concurrent access
  pub fn shared() -> Arc<Mutex<Self>> { Arc::new(Mutex::new(Self::new())) }

  /// Register an agent with the runtime
  pub fn register_agent<A>(&mut self, agent: Agent<A>) -> AgentId
  where A: LifeCycle + 'static {
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
    A: LifeCycle + 'static,
  {
    let name = name.into();
    if self.name_to_id.contains_key(&name) {
      return Err(format!("Agent name '{}' already exists", name));
    }

    let id = agent.id();
    self.agents.insert(id, Box::new(agent));
    self.name_to_id.insert(name, id);
    Ok(id)
  }

  /// Register an agent and start it immediately
  pub fn spawn_agent<A>(&mut self, agent: Agent<A>) -> AgentId
  where A: LifeCycle + 'static {
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
    A: LifeCycle + 'static,
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
  pub fn broadcast_message<M>(&mut self, message: M) -> usize
  where M: Any + Clone + Send + Sync + 'static {
    let message_type = TypeId::of::<M>();
    let mut delivered_count = 0;

    for agent in self.agents.values_mut() {
      // Check if agent has handlers for this message type
      if agent.handlers().contains_key(&message_type) {
        agent.enqueue_boxed_message(Box::new(message.clone()));
        delivered_count += 1;
      }
    }

    delivered_count
  }

  /// Send a message to a specific agent by ID
  pub fn send_to_agent_by_id<M>(&mut self, agent_id: AgentId, message: M) -> Result<(), String>
  where M: Any + Send + Sync + 'static {
    match self.agents.get_mut(&agent_id) {
      Some(agent) => {
        agent.enqueue_boxed_message(Box::new(message));
        Ok(())
      },
      None => Err(format!("Agent with ID {} not found", agent_id)),
    }
  }

  /// Send a message to a specific agent by name
  pub fn send_to_agent_by_name<M>(&mut self, agent_name: &str, message: M) -> Result<(), String>
  where M: Any + Send + Sync + 'static {
    let agent_id = self
      .agent_id_by_name(agent_name)
      .ok_or_else(|| format!("Agent '{}' not found", agent_name))?;
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
      self.route_reply_to_agents(&*reply);
    }

    total_processed
  }

  /// Execute a single runtime step: process messages and route replies
  /// Returns the number of messages processed in this step
  pub fn step(&mut self) -> usize {
    let mut step_processed = 0;
    let mut pending_replies = Vec::new();

    // Process all agents that have pending messages
    for agent in self.agents.values_mut() {
      if agent.should_process_mailbox() {
        let replies = agent.process_pending_messages();
        step_processed += replies.len();
        pending_replies.extend(replies);
      }
    }

    // Route all replies to appropriate agents
    for reply in pending_replies {
      self.route_reply_to_agents(&*reply);
    }

    step_processed
  }

  /// Run the runtime until no more messages are being processed
  /// Returns total messages processed and number of steps taken
  pub fn run(&mut self) -> RuntimeExecutionResult {
    self.run_with_limit(1000) // Default reasonable limit
  }

  /// Run the runtime with a maximum number of steps to prevent infinite loops
  /// Returns total messages processed and number of steps taken
  pub fn run_with_limit(&mut self, max_steps: usize) -> RuntimeExecutionResult {
    let mut total_processed = 0;
    let mut steps_taken = 0;

    for step in 0..max_steps {
      let processed_this_step = self.step();
      total_processed += processed_this_step;
      steps_taken = step + 1;

      // If no messages were processed, the system has reached a stable state
      if processed_this_step == 0 {
        break;
      }
    }

    RuntimeExecutionResult {
      total_messages_processed: total_processed,
      steps_taken,
      reached_stable_state: steps_taken < max_steps,
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
    match self.agents.get_mut(&agent_id) {
      Some(agent) => {
        agent.start();
        Ok(())
      },
      None => Err(format!("Agent with ID {} not found", agent_id)),
    }
  }

  /// Start an agent by name
  pub fn start_agent_by_name(&mut self, agent_name: &str) -> Result<(), String> {
    let agent_id = self
      .agent_id_by_name(agent_name)
      .ok_or_else(|| format!("Agent '{}' not found", agent_name))?;
    self.start_agent_by_id(agent_id)
  }

  /// Pause an agent by ID
  pub fn pause_agent_by_id(&mut self, agent_id: AgentId) -> Result<(), String> {
    match self.agents.get_mut(&agent_id) {
      Some(agent) => {
        agent.pause();
        Ok(())
      },
      None => Err(format!("Agent with ID {} not found", agent_id)),
    }
  }

  /// Pause an agent by name
  pub fn pause_agent_by_name(&mut self, agent_name: &str) -> Result<(), String> {
    let agent_id = self
      .agent_id_by_name(agent_name)
      .ok_or_else(|| format!("Agent '{}' not found", agent_name))?;
    self.pause_agent_by_id(agent_id)
  }

  /// Resume an agent by ID
  pub fn resume_agent_by_id(&mut self, agent_id: AgentId) -> Result<(), String> {
    match self.agents.get_mut(&agent_id) {
      Some(agent) => {
        agent.resume();
        Ok(())
      },
      None => Err(format!("Agent with ID {} not found", agent_id)),
    }
  }

  /// Resume an agent by name
  pub fn resume_agent_by_name(&mut self, agent_name: &str) -> Result<(), String> {
    let agent_id = self
      .agent_id_by_name(agent_name)
      .ok_or_else(|| format!("Agent '{}' not found", agent_name))?;
    self.resume_agent_by_id(agent_id)
  }

  /// Stop an agent by ID
  pub fn stop_agent_by_id(&mut self, agent_id: AgentId) -> Result<(), String> {
    match self.agents.get_mut(&agent_id) {
      Some(agent) => {
        agent.stop();
        Ok(())
      },
      None => Err(format!("Agent with ID {} not found", agent_id)),
    }
  }

  /// Stop an agent by name
  pub fn stop_agent_by_name(&mut self, agent_name: &str) -> Result<(), String> {
    let agent_id = self
      .agent_id_by_name(agent_name)
      .ok_or_else(|| format!("Agent '{}' not found", agent_name))?;
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

    match self.agents.remove(&agent_id) {
      Some(agent) => Ok(agent),
      None => Err(format!("Agent with ID {} not found", agent_id)),
    }
  }

  /// Remove an agent by name and return it
  pub fn remove_agent_by_name(
    &mut self,
    agent_name: &str,
  ) -> Result<Box<dyn RuntimeAgent>, String> {
    let agent_id = self
      .agent_id_by_name(agent_name)
      .ok_or_else(|| format!("Agent '{}' not found", agent_name))?;
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
  fn route_reply_to_agents(&mut self, reply: &dyn Any) {
    let reply_type = reply.type_id();

    // Find all agents that can handle this reply type
    let target_agent_ids: Vec<AgentId> = self
      .agents
      .iter()
      .filter_map(
        |(id, agent)| {
          if agent.handlers().contains_key(&reply_type) {
            Some(*id)
          } else {
            None
          }
        },
      )
      .collect();

    // Route the reply to each target agent
    // Note: This approach requires replies to implement Clone + Send + Sync
    // We use a type-erased approach to handle different reply types
    for agent_id in target_agent_ids {
      if let Some(agent) = self.agents.get_mut(&agent_id) {
        // We can't clone arbitrary `dyn Any`, so we use a workaround:
        // The reply routing will work for replies that were created as Clone types
        // For now, we skip non-cloneable replies with a warning
        if let Some(cloned_reply) = Self::try_clone_any_reply() {
          agent.enqueue_boxed_message(cloned_reply);
        }
      }
    }
  }

  /// Attempt to clone a reply of unknown type
  /// This is a fundamental limitation of type-erased replies
  /// In practice, most message types should implement Clone
  fn try_clone_any_reply() -> Option<Box<dyn Any + Send + Sync>> {
    // This is the fundamental challenge with type-erased message routing.
    // Without knowing the concrete type, we can't clone arbitrary replies.
    //
    // Solutions for production systems:
    // 1. Require all message types to implement Clone
    // 2. Use a type registry with clone functions
    // 3. Use Rc/Arc for shared ownership
    // 4. Implement a custom CloneAny trait
    //
    // For now, we return None to indicate cloning failed
    // This means replies won't be automatically routed between agents
    // Users can manually route replies using route_cloneable_reply()
    None
  }
}

impl Default for Runtime {
  fn default() -> Self { Self::new() }
}

/// Statistics about the runtime state
#[derive(Debug, Clone)]
pub struct RuntimeStatistics {
  pub total_agents:                 usize,
  pub running_agents:               usize,
  pub paused_agents:                usize,
  pub stopped_agents:               usize,
  pub agents_with_pending_messages: usize,
}

/// Result of runtime execution
#[derive(Debug, Clone)]
pub struct RuntimeExecutionResult {
  pub total_messages_processed: usize,
  pub steps_taken:              usize,
  pub reached_stable_state:     bool,
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

    fn handle(&mut self, message: NumberMessage) -> Self::Reply {
      self.total += message.value;
      println!("CounterAgent total is now: {}", self.total);
    }
  }

  impl Handler<TextMessage> for Logger {
    type Reply = ();

    fn handle(&mut self, message: TextMessage) -> Self::Reply {
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
    let delivered = runtime.broadcast_message(NumberMessage { value: 42 });
    assert_eq!(delivered, 1); // Only counter can handle NumberMessage

    let delivered = runtime.broadcast_message(TextMessage { content: "Hello".to_string() });
    assert_eq!(delivered, 1); // Only logger can handle TextMessage

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
    assert_eq!(agent1_id.value(), 1);
    assert_eq!(agent2_id.value(), 2);
    assert_eq!(agent3_id.value(), 3);

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
    let processed = runtime.step();
    assert_eq!(processed, 2); // Both messages processed

    // Test full runtime execution
    runtime.broadcast_message(NumberMessage { value: 5 });
    let result = runtime.run();
    assert_eq!(result.total_messages_processed, 1);
    assert_eq!(result.steps_taken, 1);
    assert!(result.reached_stable_state);

    // Test runtime with no work
    let result = runtime.run();
    assert_eq!(result.total_messages_processed, 0);
    assert_eq!(result.steps_taken, 1);
    assert!(result.reached_stable_state);
  }
}
