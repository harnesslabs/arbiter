use std::{
  any::{Any, TypeId},
  collections::{HashMap, VecDeque},
  sync::{Arc, Mutex, MutexGuard},
};

#[cfg(feature = "wasm")] use wasm_bindgen::prelude::*;

use crate::agent::{Agent, AgentState, LifeCycle, RuntimeAgent};

/// A multi-agent runtime that manages agent lifecycles and message routing
#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub struct Runtime {
  agents: HashMap<String, Box<dyn RuntimeAgent>>,
}

impl Runtime {
  /// Create a new runtime
  pub fn new() -> Self { Self { agents: HashMap::new() } }

  /// Create a shared runtime wrapped in Arc<Mutex<>> for concurrent access
  pub fn shared() -> Arc<Mutex<Self>> { Arc::new(Mutex::new(Self::new())) }

  /// Register an agent with the runtime
  pub fn register_agent<A>(&mut self, name: impl Into<String>, agent: A) -> Result<(), String>
  where A: LifeCycle + 'static {
    let name = name.into();
    if self.agents.contains_key(&name) {
      return Err(format!("Agent '{}' already exists", name));
    }

    let agent_container = Agent::new(agent);
    self.agents.insert(name, Box::new(agent_container));
    Ok(())
  }

  /// Register an agent with handlers and start it immediately
  pub fn spawn_agent<A>(&mut self, name: &str, agent: A) -> Result<(), String>
  where A: LifeCycle + 'static {
    self.register_agent(name, agent)?;
    self.start_agent(name);
    Ok(())
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

  /// Send a message to a specific agent
  pub fn send_to_agent<M>(&mut self, agent_name: &str, message: M) -> Result<(), String>
  where M: Any + Send + Sync + 'static {
    match self.agents.get_mut(agent_name) {
      Some(agent) => {
        agent.enqueue_boxed_message(Box::new(message));
        Ok(())
      },
      None => Err(format!("Agent '{}' not found", agent_name)),
    }
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

    // Second pass: route all collected replies
    for reply in all_replies {
      self.route_reply_message(&*reply);
    }

    total_processed
  }

  /// Add a handler to an existing agent at runtime
  pub fn add_handler_to_agent<M>(&mut self, agent_name: &str) -> Result<(), String>
  where M: Clone + Send + Sync + 'static {
    // This is a limitation - we can't dynamically add handlers without knowing the agent type
    // We'd need a different architecture for this. For now, return an error.
    Err(
      "Dynamic handler addition requires agent type information - not implemented yet".to_string(),
    )
  }

  /// Get list of all agent names
  pub fn agent_names(&self) -> Vec<&String> { self.agents.keys().collect() }

  /// Get agent count
  pub fn agent_count(&self) -> usize { self.agents.len() }

  /// Get count of agents that need processing
  pub fn agents_needing_processing(&self) -> usize {
    self.agents.values().filter(|agent| agent.should_process_mailbox()).count()
  }

  /// Route a cloneable reply message to all agents that can handle it
  /// This is a convenience method for when you know the reply type implements Clone
  pub fn route_cloneable_reply<R>(&mut self, reply: R) -> usize
  where R: Any + Clone + Send + Sync + 'static {
    self.broadcast_message(reply)
  }

  /// Start an agent
  pub fn start_agent(&mut self, name: &str) -> Result<(), String> {
    match self.agents.get_mut(name) {
      Some(agent) => {
        agent.start();
        Ok(())
      },
      None => Err(format!("Agent '{}' not found", name)),
    }
  }

  /// Pause an agent
  pub fn pause_agent(&mut self, name: &str) -> Result<(), String> {
    match self.agents.get_mut(name) {
      Some(agent) => {
        agent.pause();
        Ok(())
      },
      None => Err(format!("Agent '{}' not found", name)),
    }
  }

  /// Resume an agent
  pub fn resume_agent(&mut self, name: &str) -> Result<(), String> {
    match self.agents.get_mut(name) {
      Some(agent) => {
        agent.resume();
        Ok(())
      },
      None => Err(format!("Agent '{}' not found", name)),
    }
  }

  /// Stop an agent
  pub fn stop_agent(&mut self, name: &str) -> Result<(), String> {
    match self.agents.get_mut(name) {
      Some(agent) => {
        agent.stop();
        Ok(())
      },
      None => Err(format!("Agent '{}' not found", name)),
    }
  }

  /// Remove an agent from the runtime
  pub fn remove_agent(&mut self, name: &str) -> Result<(), String> {
    match self.agents.remove(name) {
      Some(_) => Ok(()),
      None => Err(format!("Agent '{}' not found", name)),
    }
  }

  /// Get the state of an agent
  pub fn agent_state(&self, name: &str) -> Option<AgentState> {
    self.agents.get(name).map(|agent| agent.state())
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

  /// Remove all agents from the runtime
  pub fn remove_all_agents(&mut self) -> usize {
    let count = self.agents.len();
    self.agents.clear();
    count
  }

  /// Get all agents by their current state
  pub fn agents_by_state(&self, state: AgentState) -> Vec<String> {
    self
      .agents
      .iter()
      .filter_map(|(name, agent)| if agent.state() == state { Some(name.clone()) } else { None })
      .collect()
  }

  /// Helper function to route reply messages back into the system
  fn route_reply_message(&self, reply: &dyn Any) {
    let reply_type = reply.type_id();

    // Collect agents that can handle this reply type first
    let mut target_agents = Vec::new();
    for (name, agent) in &self.agents {
      if agent.handlers().contains_key(&reply_type) {
        target_agents.push(name.clone());
      }
    }

    // Then send the reply to each target agent
    // Note: This is a limitation - we can only route replies that implement Clone
    // For now, we skip reply routing since we can't clone arbitrary boxed types
    // In a production system, you'd want a proper message cloning registry
    for _agent_name in target_agents {
      // TODO: Implement proper reply cloning and routing
      // For now, we skip this to avoid the complexity
    }
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
  struct CounterAgent {
    total: i32,
  }

  impl LifeCycle for CounterAgent {}

  #[derive(Clone)]
  struct LogAgent {
    name:          String,
    message_count: i32,
  }

  impl LifeCycle for LogAgent {}

  impl Handler<NumberMessage> for CounterAgent {
    type Reply = ();

    fn handle(&mut self, message: NumberMessage) -> Self::Reply {
      self.total += message.value;
      println!("CounterAgent total is now: {}", self.total);
    }
  }

  impl Handler<TextMessage> for LogAgent {
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

    // Register agents with handlers
    let counter = CounterAgent { total: 0 };
    let logger = LogAgent { name: "TestLogger".to_string(), message_count: 0 };

    runtime.register_agent("counter", counter).unwrap();
    runtime.register_agent("logger", logger).unwrap();

    assert_eq!(runtime.agent_count(), 2);
    assert_eq!(runtime.agent_names().len(), 2);
  }

  #[test]
  fn test_message_routing() {
    let mut runtime = Runtime::new();

    // Register agents with specific handlers
    let counter = Agent::new(CounterAgent { total: 0 }).with_handler::<NumberMessage>();
    let logger = Agent::new(LogAgent { name: "TestLogger".to_string(), message_count: 0 })
      .with_handler::<TextMessage>();

    runtime.agents.insert("counter".to_string(), Box::new(counter));
    runtime.agents.insert("logger".to_string(), Box::new(logger));

    // Start agents
    runtime.start_agent("counter").unwrap();
    runtime.start_agent("logger").unwrap();

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

    let counter = CounterAgent { total: 0 };
    runtime.spawn_agent("counter", counter).unwrap();

    // Agent should be running after spawn
    assert_eq!(runtime.agent_state("counter"), Some(AgentState::Running));

    // Pause the agent
    runtime.pause_agent("counter").unwrap();
    assert_eq!(runtime.agent_state("counter"), Some(AgentState::Paused));

    // Resume the agent
    runtime.resume_agent("counter").unwrap();
    assert_eq!(runtime.agent_state("counter"), Some(AgentState::Running));

    // Stop the agent
    runtime.stop_agent("counter").unwrap();
    assert_eq!(runtime.agent_state("counter"), Some(AgentState::Stopped));
  }

  #[test]
  fn test_runtime_statistics() {
    let mut runtime = Runtime::new();

    runtime.spawn_agent("agent1", CounterAgent { total: 0 }).unwrap();
    runtime.spawn_agent("agent2", CounterAgent { total: 0 }).unwrap();
    runtime.register_agent("agent3", CounterAgent { total: 0 }).unwrap();

    let stats = runtime.statistics();
    assert_eq!(stats.total_agents, 3);
    assert_eq!(stats.running_agents, 2); // agent1 and agent2 were spawned (started)
    assert_eq!(stats.stopped_agents, 1); // agent3 was only registered
  }

  #[test]
  fn test_bulk_agent_operations() {
    let mut runtime = Runtime::new();

    // Register multiple agents
    runtime.register_agent("agent1", CounterAgent { total: 0 }).unwrap();
    runtime.register_agent("agent2", CounterAgent { total: 0 }).unwrap();
    runtime.register_agent("agent3", CounterAgent { total: 0 }).unwrap();

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
    assert!(stopped_agents.contains(&"agent1".to_string()));
    assert!(stopped_agents.contains(&"agent2".to_string()));
    assert!(stopped_agents.contains(&"agent3".to_string()));

    // Remove all agents
    let removed = runtime.remove_all_agents();
    assert_eq!(removed, 3);
    assert_eq!(runtime.agent_count(), 0);
  }
}
