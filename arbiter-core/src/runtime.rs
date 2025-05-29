use std::{
  any::{Any, TypeId},
  collections::HashMap,
};

use crate::{
  agent::{Agent, Context},
  handler::Handler,
};

// Simple runtime that can hold different types of agents
pub struct Runtime {
  agents:   HashMap<String, Box<dyn Any + Send + Sync>>,
  handlers: HashMap<TypeId, Box<dyn Handler>>,
}

impl Runtime {
  pub fn new() -> Self { Self { agents: HashMap::new() } }

  // Register any agent that implements the Agent trait
  pub fn register_agent<A: Agent + Send + Sync + 'static>(&mut self, name: String, agent: A) {
    let context = Context::new(agent);
    self.agents.insert(name, Box::new(context));
  }

  // Send a message to a specific agent
  pub fn send_message<M: 'static>(&self, agent_name: &str, message: M) -> bool
  where M: Send + Sync {
    if let Some(context_box) = self.agents.get(agent_name) {
      // Try to downcast to Context<A> where A implements Handler<M>
      // This is a bit tricky since we don't know A at compile time
      // For now, return true to indicate we found the agent
      true
    } else {
      false
    }
  }

  // Broadcast a message to all agents
  pub fn broadcast_message<M: 'static>(&self, _message: M)
  where M: Send + Sync + Clone {
    // For now, just iterate through agents
    for _context in self.agents.values() {
      // Would need to try downcasting each one
    }
  }

  // Get list of registered agent names
  pub fn list_agents(&self) -> Vec<&String> { self.agents.keys().collect() }

  // Get agent info
  pub fn get_agent_info(&self, name: &str) -> Option<&'static str> {
    if self.agents.contains_key(name) {
      Some("Agent") // Simplified for now
    } else {
      None
    }
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

  // Example agent types
  struct LogAgent {
    name:          String,
    message_count: i32,
  }

  impl Agent for LogAgent {}

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
  fn test_runtime_basic() {
    let mut runtime = Runtime::new();

    // Register different types of agents
    runtime.register_agent("logger".to_string(), LogAgent {
      name:          "Logger1".to_string(),
      message_count: 0,
    });

    runtime.register_agent("counter".to_string(), CounterAgent { total: 0 });

    // For now, just test that we can register agents
    assert_eq!(runtime.list_agents().len(), 2);
    assert!(runtime.get_agent_info("logger").is_some());
    assert!(runtime.get_agent_info("counter").is_some());
  }
}
