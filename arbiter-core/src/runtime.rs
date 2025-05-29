use std::{any::Any, collections::HashMap};

// Common message trait that all messages must implement
pub trait Message: Send + Sync {
  fn as_any(&self) -> &dyn Any;
  fn message_type(&self) -> &'static str;
}

// Agent trait that can receive any message
pub trait Agent: Send + Sync {
  fn handle_message(&mut self, message: &dyn Message);
  fn agent_type(&self) -> &'static str;
}

// Simple runtime that can hold different types of agents
pub struct Runtime {
  agents: HashMap<String, Box<dyn Agent>>,
}

impl Runtime {
  pub fn new() -> Self { Self { agents: HashMap::new() } }

  // Register any agent that implements the Agent trait
  pub fn register_agent(&mut self, name: String, agent: Box<dyn Agent>) {
    self.agents.insert(name, agent);
  }

  // Send a message to a specific agent
  pub fn send_message(&mut self, agent_name: &str, message: &dyn Message) -> bool {
    if let Some(agent) = self.agents.get_mut(agent_name) {
      agent.handle_message(message);
      true
    } else {
      false
    }
  }

  // Broadcast a message to all agents
  pub fn broadcast_message(&mut self, message: &dyn Message) {
    for agent in self.agents.values_mut() {
      agent.handle_message(message);
    }
  }

  // Get list of registered agent names
  pub fn list_agents(&self) -> Vec<&String> { self.agents.keys().collect() }

  // Get agent info
  pub fn get_agent_info(&self, name: &str) -> Option<&'static str> {
    self.agents.get(name).map(|agent| agent.agent_type())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  // Example message types
  #[derive(Debug)]
  struct TextMessage {
    content: String,
  }

  impl Message for TextMessage {
    fn as_any(&self) -> &dyn Any { self }

    fn message_type(&self) -> &'static str { "TextMessage" }
  }

  #[derive(Debug)]
  struct NumberMessage {
    value: i32,
  }

  impl Message for NumberMessage {
    fn as_any(&self) -> &dyn Any { self }

    fn message_type(&self) -> &'static str { "NumberMessage" }
  }

  // Example agent types
  struct LogAgent {
    name:          String,
    message_count: i32,
  }

  impl Agent for LogAgent {
    fn handle_message(&mut self, message: &dyn Message) {
      self.message_count += 1;

      // Handle different message types
      if let Some(text_msg) = message.as_any().downcast_ref::<TextMessage>() {
        println!("LogAgent '{}' received text: {}", self.name, text_msg.content);
      } else if let Some(num_msg) = message.as_any().downcast_ref::<NumberMessage>() {
        println!("LogAgent '{}' received number: {}", self.name, num_msg.value);
      } else {
        println!(
          "LogAgent '{}' received unknown message type: {}",
          self.name,
          message.message_type()
        );
      }
    }

    fn agent_type(&self) -> &'static str { "LogAgent" }
  }

  struct CounterAgent {
    total: i32,
  }

  impl Agent for CounterAgent {
    fn handle_message(&mut self, message: &dyn Message) {
      // Only handle number messages
      if let Some(num_msg) = message.as_any().downcast_ref::<NumberMessage>() {
        self.total += num_msg.value;
        println!("CounterAgent total is now: {}", self.total);
      }
    }

    fn agent_type(&self) -> &'static str { "CounterAgent" }
  }

  #[test]
  fn test_runtime_multi_type() {
    let mut runtime = Runtime::new();

    // Register different types of agents
    runtime.register_agent(
      "logger".to_string(),
      Box::new(LogAgent { name: "Logger1".to_string(), message_count: 0 }),
    );

    runtime.register_agent("counter".to_string(), Box::new(CounterAgent { total: 0 }));

    // Create different message types
    let text_msg = TextMessage { content: "Hello World!".to_string() };
    let num_msg = NumberMessage { value: 42 };

    // Send different messages to different agents
    assert!(runtime.send_message("logger", &text_msg));
    assert!(runtime.send_message("logger", &num_msg));
    assert!(runtime.send_message("counter", &num_msg));

    // Try sending to non-existent agent
    assert!(!runtime.send_message("missing", &text_msg));

    // Broadcast to all agents
    let broadcast_msg = NumberMessage { value: 10 };
    runtime.broadcast_message(&broadcast_msg);

    // Verify agents are registered
    assert_eq!(runtime.list_agents().len(), 2);
    assert_eq!(runtime.get_agent_info("logger"), Some("LogAgent"));
    assert_eq!(runtime.get_agent_info("counter"), Some("CounterAgent"));
  }
}
