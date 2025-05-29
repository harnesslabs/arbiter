use std::{
  any::{Any, TypeId},
  collections::HashMap,
  marker::PhantomData,
};

use crate::{
  agent::{Agent, Context},
  handler::{Handler, HandlerWrapper, MessageHandler},
};

// Simple runtime that can hold different types of agents
pub struct Runtime {
  agents:   HashMap<String, Box<dyn Any + Send + Sync>>,
  handlers: HashMap<TypeId, Vec<Box<dyn MessageHandler>>>,
}

impl Runtime {
  pub fn new() -> Self { Self { agents: HashMap::new(), handlers: HashMap::new() } }

  // Register any agent that implements the Agent trait
  pub fn register_agent<A: Agent + Send + Sync + 'static>(&mut self, name: String, agent: A) {
    let context = Context::new(agent);
    self.agents.insert(name, Box::new(context));
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

  // Send a message to all handlers that can handle this message type
  pub fn send_message<M: Any + Clone + Send + Sync>(&mut self, message: M) -> bool {
    let type_id = TypeId::of::<M>();
    if let Some(handlers) = self.handlers.get_mut(&type_id) {
      let has_handlers = !handlers.is_empty();
      for handler in handlers {
        handler.handle_message(&message);
      }
      has_handlers
    } else {
      false
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

    // Register handlers for message types
    runtime.register_handler(LogAgent { name: "Handler1".to_string(), message_count: 0 });
    runtime.register_handler(CounterAgent { total: 0 });

    // Send messages - they'll go to all handlers that can handle this message type
    let num_msg = NumberMessage { value: 42 };
    assert!(runtime.send_message(num_msg));

    // Test that we can register agents
    assert_eq!(runtime.list_agents().len(), 2);
    assert!(runtime.get_agent_info("logger").is_some());
    assert!(runtime.get_agent_info("counter").is_some());
  }
}
