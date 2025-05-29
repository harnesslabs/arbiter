use std::{
  any::{Any, TypeId},
  collections::{HashMap, VecDeque},
  marker::PhantomData,
};

use crate::{
  agent::{Agent, AgentContainer, AgentState},
  handler::{Handler, HandlerWrapper, MessageHandler},
};

// Enhanced runtime with agent lifecycle management and mailboxes
pub struct Runtime {
  agents:          HashMap<String, Box<dyn RuntimeAgent>>,
  handlers:        HashMap<TypeId, Vec<Box<dyn MessageHandler>>>,
  message_queue:   VecDeque<Box<dyn Any + Send + Sync>>,
  agent_mailboxes: HashMap<String, VecDeque<Box<dyn Any + Send + Sync>>>, /* Messages for paused
                                                                           * agents */
  max_iterations:  usize, // Prevent infinite loops
}

// Trait for runtime-manageable agents
pub trait RuntimeAgent: Send + Sync {
  fn start(&mut self);
  fn pause(&mut self);
  fn stop(&mut self);
  fn resume(&mut self);
  fn state(&self) -> AgentState;
  fn is_active(&self) -> bool;
  fn agent_type_name(&self) -> &'static str;
}

// Implement RuntimeAgent for AgentContainer<T>
impl<A: Agent> RuntimeAgent for AgentContainer<A> {
  fn start(&mut self) { AgentContainer::start(self); }

  fn pause(&mut self) { AgentContainer::pause(self); }

  fn stop(&mut self) { AgentContainer::stop(self); }

  fn resume(&mut self) { AgentContainer::resume(self); }

  fn state(&self) -> AgentState { self.state().clone() }

  fn is_active(&self) -> bool { AgentContainer::is_active(self) }

  fn agent_type_name(&self) -> &'static str { std::any::type_name::<A>() }
}

impl Runtime {
  pub fn new() -> Self {
    Self {
      agents:          HashMap::new(),
      handlers:        HashMap::new(),
      message_queue:   VecDeque::new(),
      agent_mailboxes: HashMap::new(),
      max_iterations:  1000, // Reasonable default
    }
  }

  pub fn with_max_iterations(mut self, max_iterations: usize) -> Self {
    self.max_iterations = max_iterations;
    self
  }

  // Register any agent that implements the Agent trait
  pub fn register_agent<A: Agent>(&mut self, name: String, agent: A) -> bool {
    if self.agents.contains_key(&name) {
      return false; // Agent name already exists
    }

    let container = AgentContainer::new(agent);
    self.agents.insert(name.clone(), Box::new(container));
    self.agent_mailboxes.insert(name, VecDeque::new());
    true
  }

  // Add agent and immediately start it
  pub fn add_and_start_agent<A: Agent>(&mut self, name: String, agent: A) -> bool {
    if self.register_agent(name.clone(), agent) {
      self.start_agent(&name)
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
    self.handlers.entry(type_id).or_insert_with(Vec::new).push(Box::new(wrapped));
  }

  // Send a message - adds to queue for processing
  pub fn send_message<M: Any + Clone + Send + Sync + 'static>(&mut self, message: M) {
    self.message_queue.push_back(Box::new(message));
  }

  // Process a single message and add any replies to the queue
  fn process_message(&mut self, message: Box<dyn Any + Send + Sync>) -> bool {
    let type_id = (&*message).type_id();

    if let Some(handlers) = self.handlers.get_mut(&type_id) {
      let mut processed = false;
      for handler in handlers {
        let reply = handler.handle_message(&*message);

        // Add the reply to the message queue for further processing
        // Skip () replies as they indicate no further processing needed
        if (&*reply).type_id() != TypeId::of::<()>() {
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
      if iterations >= self.max_iterations {
        println!("Warning: Maximum iterations ({}) reached, stopping runtime", self.max_iterations);
        break;
      }

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
    if let Some(agent) = self.agents.get_mut(name) {
      agent.start();
      true
    } else {
      false
    }
  }

  pub fn pause_agent(&mut self, name: &str) -> bool {
    if let Some(agent) = self.agents.get_mut(name) {
      agent.pause();
      true
    } else {
      false
    }
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
    if let Some(agent) = self.agents.get_mut(name) {
      agent.resume();
      true
    } else {
      false
    }
  }

  // Get agent state
  pub fn get_agent_state(&self, name: &str) -> Option<AgentState> {
    if let Some(agent) = self.agents.get(name) {
      Some(agent.state())
    } else {
      None
    }
  }

  // Get list of registered agent names
  pub fn list_agents(&self) -> Vec<&String> { self.agents.keys().collect() }

  // Get agent info
  pub fn get_agent_info(&self, name: &str) -> Option<&'static str> {
    if let Some(agent) = self.agents.get(name) {
      Some(agent.agent_type_name())
    } else {
      None
    }
  }

  // Check if message queue is empty
  pub fn is_idle(&self) -> bool {
    self.message_queue.is_empty() && self.agent_mailboxes.values().all(|mb| mb.is_empty())
  }

  // Get current queue length
  pub fn queue_length(&self) -> usize { self.message_queue.len() }

  // Get total messages in all mailboxes
  pub fn total_mailbox_messages(&self) -> usize {
    self.agent_mailboxes.values().map(|mb| mb.len()).sum()
  }
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
