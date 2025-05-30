use std::{
  any::{Any, TypeId},
  collections::{HashMap, VecDeque},
  marker::PhantomData,
  sync::{Arc, Mutex, MutexGuard},
};

#[cfg(feature = "wasm")] use wasm_bindgen::prelude::*;

use crate::{
  agent::{Agent, AgentState, RuntimeAgent, SharedAgentWrapper},
  handler::{Handler, HandlerWrapper, MessageHandler},
};

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
  agents:          HashMap<String, Box<dyn RuntimeAgent>>,
  handlers:        HashMap<TypeId, Vec<Box<dyn MessageHandler>>>,
  message_queue:   VecDeque<Box<dyn Any + Send + Sync>>,
  agent_mailboxes: HashMap<String, VecDeque<Box<dyn Any + Send + Sync>>>, /* Messages for paused
                                                                           * agents */
}

impl Runtime {
  pub fn new() -> Self {
    Self {
      agents:          HashMap::new(),
      handlers:        HashMap::new(),
      message_queue:   VecDeque::new(),
      agent_mailboxes: HashMap::new(),
    }
  }

  // Register any agent that implements the Agent trait
  pub fn register_agent<A: Agent>(&mut self, name: String, agent: A) -> bool {
    if self.agents.contains_key(&name) {
      return false; // Agent name already exists
    }

    let container = crate::agent::AgentContainer::new(agent);
    self.agents.insert(name.clone(), Box::new(container));
    self.agent_mailboxes.insert(name, VecDeque::new());
    true
  }

  // Register a shared agent (for when you need to create handlers for it)
  pub fn register_shared_agent<A: Agent>(
    &mut self,
    name: String,
    shared_agent: crate::agent::SharedAgent<A>,
  ) -> bool {
    if self.agents.contains_key(&name) {
      return false; // Agent name already exists
    }

    // We need to extract the container to register as RuntimeAgent
    // This is a bit tricky with the shared ownership, but we can clone the Arc
    // and create a wrapper that delegates to the shared agent
    let wrapper = SharedAgentWrapper { shared_agent };
    self.agents.insert(name.clone(), Box::new(wrapper));
    self.agent_mailboxes.insert(name, VecDeque::new());
    true
  }

  // Add agent and immediately start it
  pub fn add_and_start_agent<A: Agent>(&mut self, name: &str, agent: A) -> bool {
    if self.register_agent(name.to_string(), agent) {
      self.start_agent(name)
    } else {
      false
    }
  }

  // Remove an agent from the runtime
  pub fn remove_agent(&mut self, name: &str) -> bool {
    if self.agents.remove(name).is_some() {
      self.agent_mailboxes.remove(name);
      true
    } else {
      false
    }
  }

  // Register a handler for a specific message type
  pub fn register_handler<H, M>(&mut self, handler: H)
  where
    H: Handler<M> + Send + Sync + 'static,
    H::Reply: Send + Sync + 'static,
    M: Any + Clone + Send + Sync + 'static, {
    let type_id = TypeId::of::<M>();
    let wrapped = HandlerWrapper { handler, _phantom: PhantomData };
    self.handlers.entry(type_id).or_default().push(Box::new(wrapped));
  }

  // Register an agent handler for a specific message type
  pub fn register_agent_handler<A, M>(&mut self, handler: crate::agent::AgentHandler<A, M>)
  where
    A: Agent + Handler<M>,
    M: Clone + Send + Sync + 'static,
    A::Reply: Send + Sync + 'static, {
    let type_id = TypeId::of::<M>();
    self.handlers.entry(type_id).or_default().push(Box::new(handler));
  }

  // Send a message - adds to queue for processing
  pub fn send_message<M: Any + Clone + Send + Sync + 'static>(&mut self, message: M) {
    self.message_queue.push_back(Box::new(message));
  }

  // Process a single message and add any replies to the queue
  fn process_message(&mut self, message: Box<dyn Any + Send + Sync>) -> bool {
    let type_id = (*message).type_id();

    if let Some(handlers) = self.handlers.get_mut(&type_id) {
      let mut processed = false;
      for handler in handlers {
        let reply = handler.handle_message(&*message);

        // Add the reply to the message queue for further processing
        // Skip () replies as they indicate no further processing needed
        if (*reply).type_id() != TypeId::of::<()>() {
          self.message_queue.push_back(reply);
        }
        processed = true;
      }
      processed
    } else {
      false
    }
  }

  // Process mailboxes for resumed agents (simplified for now)
  fn process_agent_mailboxes(&mut self) {
    // For now, we'll skip the complex mailbox processing
    // In a full implementation, this would deliver queued messages to active agents
    // The challenge is that we need type information to properly route messages
    // to specific agent handlers, which requires a different architecture
  }

  // Run the runtime - processes all messages until queue is empty or max iterations reached
  pub fn run(&mut self) -> usize {
    let mut iterations = 0;

    while let Some(message) = self.message_queue.pop_front() {
      let processed = self.process_message(message);
      if processed {
        iterations += 1;
      }

      // Process agent mailboxes periodically
      if iterations % 10 == 0 {
        self.process_agent_mailboxes();
      }
    }

    // Final mailbox processing
    self.process_agent_mailboxes();
    iterations
  }

  // Send a message and immediately run until completion
  pub fn send_and_run<M: Any + Clone + Send + Sync + 'static>(&mut self, message: M) -> usize {
    self.send_message(message);
    self.run()
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
    if let Some(agent) = self.agents.get_mut(name) {
      agent.stop();
      // Clear the agent's mailbox when stopped
      if let Some(mailbox) = self.agent_mailboxes.get_mut(name) {
        mailbox.clear();
      }
      true
    } else {
      false
    }
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

  // Check if message queue is empty
  pub fn is_idle(&self) -> bool {
    self.message_queue.is_empty()
      && self.agent_mailboxes.values().all(std::collections::VecDeque::is_empty)
  }

  // Get current queue length
  pub fn queue_length(&self) -> usize { self.message_queue.len() }

  // Get total messages in all mailboxes
  pub fn total_mailbox_messages(&self) -> usize {
    self.agent_mailboxes.values().map(std::collections::VecDeque::len).sum()
  }

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
    self.agent_mailboxes.clear();
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

  impl Agent for LogAgent {}

  #[derive(Clone)]
  struct CounterAgent {
    total: i32,
  }

  impl Agent for CounterAgent {}

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
    let mut runtime = Runtime::new();

    // Register different types of agents - they get wrapped in containers automatically
    runtime.register_agent("logger".to_string(), LogAgent {
      name:          "Logger1".to_string(),
      message_count: 0,
    });

    runtime.register_agent("counter".to_string(), CounterAgent { total: 0 });

    // Register standalone handlers for message types
    runtime.register_handler(LogAgent { name: "Handler1".to_string(), message_count: 0 });
    runtime.register_handler(CounterAgent { total: 0 });

    // Send messages - they'll go to all handlers that can handle this message type
    let num_msg = NumberMessage { value: 42 };
    runtime.send_message(num_msg);

    // Test that we can register agents
    assert_eq!(runtime.list_agents().len(), 2);
    assert!(runtime.get_agent_info("logger").is_some());
    assert!(runtime.get_agent_info("counter").is_some());
  }
}
