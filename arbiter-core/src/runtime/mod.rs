use std::{
  any::{Any, TypeId},
  collections::{HashMap, VecDeque},
  sync::{Arc, Mutex, MutexGuard},
};

#[cfg(feature = "wasm")] use wasm_bindgen::prelude::*;

use crate::agent::{Agent, AgentState, LifeCycle, RuntimeAgent};

#[cfg_attr(feature = "wasm", wasm_bindgen)]
#[derive(Clone, Default)]
pub struct Domain {
  runtime: Arc<Mutex<Runtime>>,
}

impl Domain {
  pub fn new() -> Self { Self { runtime: Arc::new(Mutex::new(Runtime::new())) } }

  /// Get a reference to the runtime Arc for sharing
  pub fn try_lock(
    &self,
  ) -> Result<MutexGuard<'_, Runtime>, std::sync::TryLockError<MutexGuard<'_, Runtime>>> {
    self.runtime.try_lock()
  }
}

// Enhanced runtime with agent lifecycle management and mailboxes
#[derive(Default)]
pub struct Runtime {
  agents:        HashMap<String, Box<dyn RuntimeAgent>>,
  message_queue: VecDeque<Box<dyn Any + Send + Sync>>,
}

impl Runtime {
  pub fn new() -> Self { Self { agents: HashMap::new(), message_queue: VecDeque::new() } }

  // Register any agent that implements the Agent trait
  pub fn register_agent<A: LifeCycle>(&mut self, name: &str, agent: A) -> bool {
    if self.agents.contains_key(name) {
      return false; // Agent name already exists
    }

    let container = Agent::new(agent);
    self.agents.insert(name.to_string(), Box::new(container));
    true
  }

  // Register a shared agent (for when you need to create handlers for it)
  pub fn register_shared_agent<A: LifeCycle>(&mut self, name: &str, agent: A) -> bool {
    if self.agents.contains_key(name) {
      return false; // Agent name already exists
    }

    let wrapper = Agent::new(agent);
    self.agents.insert(name.to_string(), Box::new(wrapper));
    true
  }

  // Add agent and immediately start it
  pub fn add_and_start_agent<A: LifeCycle>(&mut self, name: &str, agent: A) -> bool {
    if self.register_agent(name, agent) {
      self.start_agent(name)
    } else {
      false
    }
  }

  // Remove an agent from the runtime
  pub fn remove_agent(&mut self, name: &str) -> bool { self.agents.remove(name).is_some() }

  // Send a message - adds to queue for processing
  pub fn send_message<M: Any + Clone + Send + Sync + 'static>(&mut self, message: M) {
    self.message_queue.push_back(Box::new(message));
  }

  // Process a single message and add any replies to the queue
  fn process_message(&mut self, message: Box<dyn Any + Send + Sync>) -> bool {
    for agent in self.agents.values_mut() {
      let replies = agent.process_any_message(&message);
      for reply in replies {
        self.message_queue.push_back(reply);
      }
    }
    false
  }

  // Run the runtime - processes all messages until queue is empty or max iterations reached
  pub fn run(&mut self) -> usize {
    let mut iterations = 0;

    while let Some(message) = self.message_queue.pop_front() {
      let processed = self.process_message(message);
      if processed {
        iterations += 1;
      }
    }
    iterations
  }

  // Agent lifecycle management methods
  pub fn start_agent(&mut self, name: &str) -> bool {
    self.agents.get_mut(name).is_some_and(|agent| {
      agent.start();
      true
    })
  }

  pub fn pause_agent(&mut self, name: &str) -> bool {
    self.agents.get_mut(name).is_some_and(|agent| {
      agent.pause();
      true
    })
  }

  pub fn stop_agent(&mut self, name: &str) -> bool {
    self.agents.get_mut(name).is_some_and(|agent| {
      agent.stop();
      true
    })
  }

  pub fn resume_agent(&mut self, name: &str) -> bool {
    self.agents.get_mut(name).is_some_and(|agent| {
      agent.resume();
      true
    })
  }

  // Get agent state
  pub fn get_agent_state(&self, name: &str) -> Option<AgentState> {
    self.agents.get(name).map(|agent| agent.state())
  }

  // Get list of registered agent names
  pub fn list_agents(&self) -> Vec<&String> { self.agents.keys().collect() }

  // Get agent info
  pub fn get_agent_info(&self, name: &str) -> Option<&'static str> {
    self.agents.get(name).map(|agent| agent.agent_type_name())
  }

  // Get current queue length
  pub fn queue_length(&self) -> usize { self.message_queue.len() }

  // === BULK AGENT OPERATIONS ===

  /// Start all registered agents
  pub fn start_all_agents(&mut self) -> usize {
    let agent_names: Vec<String> = self.agents.keys().cloned().collect();
    let mut started = 0;
    for name in agent_names {
      if self.start_agent(&name) {
        started += 1;
      }
    }
    started
  }

  /// Pause all agents
  pub fn pause_all_agents(&mut self) -> usize {
    let agent_names: Vec<String> = self.agents.keys().cloned().collect();
    let mut paused = 0;
    for name in agent_names {
      if self.pause_agent(&name) {
        paused += 1;
      }
    }
    paused
  }

  /// Resume all agents
  pub fn resume_all_agents(&mut self) -> usize {
    let agent_names: Vec<String> = self.agents.keys().cloned().collect();
    let mut resumed = 0;
    for name in agent_names {
      if self.resume_agent(&name) {
        resumed += 1;
      }
    }
    resumed
  }

  /// Stop all agents
  pub fn stop_all_agents(&mut self) -> usize {
    let agent_names: Vec<String> = self.agents.keys().cloned().collect();
    let mut stopped = 0;
    for name in agent_names {
      if self.stop_agent(&name) {
        stopped += 1;
      }
    }
    stopped
  }

  /// Get agents by state
  pub fn get_agents_by_state(&self, state: AgentState) -> Vec<String> {
    self
      .agents
      .iter()
      .filter_map(|(name, agent)| if agent.state() == state { Some(name.clone()) } else { None })
      .collect()
  }

  /// Get count of agents by state
  pub fn count_agents_by_state(&self, state: AgentState) -> usize {
    self.agents.values().filter(|agent| agent.state() == state).count()
  }

  /// Get agent statistics
  pub fn get_agent_stats(&self) -> AgentStats {
    let total = self.agents.len();
    let running = self.count_agents_by_state(AgentState::Running);
    let paused = self.count_agents_by_state(AgentState::Paused);
    let stopped = self.count_agents_by_state(AgentState::Stopped);

    AgentStats { total, running, paused, stopped }
  }

  /// Remove all agents
  pub fn clear_all_agents(&mut self) -> usize {
    let count = self.agents.len();
    self.agents.clear();
    count
  }
}

/// Statistics about agents in the runtime
#[derive(Debug, Clone)]
pub struct AgentStats {
  pub total:   usize,
  pub running: usize,
  pub paused:  usize,
  pub stopped: usize,
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::handler::Handler;

  // Example message types - just regular structs!
  #[derive(Debug, Clone)]
  struct TextMessage {
    content: String,
  }

  #[derive(Debug, Clone)]
  struct NumberMessage {
    value: i32,
  }

  // Example agent types - simple structs with clear state
  #[derive(Clone)]
  struct LogAgent {
    name:          String,
    message_count: i32,
  }

  impl LifeCycle for LogAgent {}

  #[derive(Clone)]
  struct CounterAgent {
    total: i32,
  }

  impl LifeCycle for CounterAgent {}

  impl Handler<NumberMessage> for LogAgent {
    type Reply = ();

    fn handle(&mut self, message: NumberMessage) -> Self::Reply {
      self.message_count += 1;
      println!("LogAgent '{}' received number: {}", self.name, message.value);
    }
  }

  impl Handler<NumberMessage> for CounterAgent {
    type Reply = ();

    fn handle(&mut self, message: NumberMessage) -> Self::Reply {
      self.total += message.value;
      println!("CounterAgent total is now: {}", self.total);
    }
  }

  #[test]
  fn test_runtime_with_containers() {
    todo!("This needs re-thought.");
    // let mut runtime = Runtime::new();

    // // Register different types of agents - they get wrapped in containers automatically
    // runtime.register_agent("logger".to_string(), LogAgent {
    //   name:          "Logger1".to_string(),
    //   message_count: 0,
    // });

    // runtime.register_agent("counter".to_string(), CounterAgent { total: 0 });

    // // Register standalone handlers for message types
    // runtime.register_handler(LogAgent { name: "Handler1".to_string(), message_count: 0 });
    // runtime.register_handler(CounterAgent { total: 0 });

    // // Send messages - they'll go to all handlers that can handle this message type
    // let num_msg = NumberMessage { value: 42 };
    // runtime.send_message(num_msg);

    // // Test that we can register agents
    // assert_eq!(runtime.list_agents().len(), 2);
    // assert!(runtime.get_agent_info("logger").is_some());
    // assert!(runtime.get_agent_info("counter").is_some());
  }
}
