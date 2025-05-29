use std::{
  any::{Any, TypeId},
  collections::HashMap,
  marker::PhantomData,
  sync::{Arc, Mutex},
};

use crate::handler::HandlerWrapper;

// Simple trait for things that can be agents
pub trait Agent: Send + Sync + 'static {}

// Container that manages handlers for an agent
pub struct AgentContainer<A: Agent> {
  agent:    A,
  handlers: HashMap<TypeId, Box<dyn crate::handler::MessageHandler>>,
}

impl<A: Agent> AgentContainer<A> {
  pub fn new(agent: A) -> Self { Self { agent, handlers: HashMap::new() } }

  // Register the agent itself as a handler for message type M
  pub fn register_handler<M>(&mut self)
  where
    A: crate::handler::Handler<M> + Clone,
    M: Any + Send + Sync + Clone + 'static,
    A::Reply: Send + Sync + 'static, {
    let type_id = TypeId::of::<M>();
    let wrapped =
      crate::handler::HandlerWrapper { handler: self.agent.clone(), _phantom: PhantomData };
    self.handlers.insert(type_id, Box::new(wrapped));
  }

  // Handle a message by routing to appropriate handler
  pub fn handle_message<M>(&mut self, message: M) -> Option<Box<dyn Any>>
  where M: Any + Send + Sync + Clone + 'static {
    let type_id = TypeId::of::<M>();
    if let Some(handler) = self.handlers.get_mut(&type_id) {
      Some(handler.handle_message(&message))
    } else {
      None
    }
  }

  // Get mutable access to the underlying agent
  pub fn agent_mut(&mut self) -> &mut A { &mut self.agent }

  // Get immutable access to the underlying agent
  pub fn agent(&self) -> &A { &self.agent }
}

// TODO (autoparallel): This could be generalized to handle different kinds of locking. In which
// case we'd make this a trait and have a default implementation that uses Arc<Mutex<A>>.
pub struct Context<A> {
  agent: Arc<Mutex<A>>,
}

// Default implementation of Context for Arc<Mutex<Agent>>
impl<A> Context<A> {
  pub fn new(agent: A) -> Self { Self { agent: Arc::new(Mutex::new(agent)) } }

  pub fn with<F, R>(&self, f: F) -> R
  where F: FnOnce(&mut A) -> R {
    let mut guard = self.agent.lock().unwrap();
    f(&mut *guard)
  }

  pub fn handle_with<M>(&self, message: M) -> <A as crate::handler::Handler<M>>::Reply
  where A: crate::handler::Handler<M> {
    self.with(|agent| agent.handle(message))
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::handler::Handler;

  // Simple agent struct - just data!
  #[derive(Clone)]
  pub struct TestAgent {
    state: i32,
  }

  // Implement Agent trait
  impl Agent for TestAgent {}

  #[derive(Debug, Clone)]
  pub struct Increment(i32);
  #[derive(Debug, Clone)]
  pub struct Multiply(i32);

  // TestAgent knows how to handle different message types
  impl Handler<Increment> for TestAgent {
    type Reply = ();

    fn handle(&mut self, message: Increment) -> Self::Reply {
      println!("Handler 1 - Received message: {:?}", message);
      self.state += message.0;
      println!("Handler 1 - Updated state: {}", self.state);
    }
  }

  impl Handler<Multiply> for TestAgent {
    type Reply = i32;

    fn handle(&mut self, message: Multiply) -> Self::Reply {
      println!("Handler 2 - Multiplying state {} by {}", self.state, message.0);
      self.state *= message.0;
      println!("Handler 2 - Updated state: {}", self.state);
      self.state
    }
  }

  #[test]
  fn test_agent_container() {
    // Create simple agent
    let test_agent = TestAgent { state: 1 };

    // Wrap in container and register handlers
    let mut container = AgentContainer::new(test_agent);
    container.register_handler::<Increment>();
    container.register_handler::<Multiply>();

    // Send messages through container
    let increment = Increment(5);
    container.handle_message(increment);

    let multiply = Multiply(3);
    let result = container.handle_message(multiply);

    // Check final state
    assert_eq!(container.agent().state, 18); // (1 + 5) * 3 = 18
  }

  #[test]
  fn test_context_direct() {
    let agent = TestAgent { state: 1 };
    let context = Context::new(agent);

    // Use first handler - agent handles Increment messages
    let increment = Increment(5);
    context.handle_with(increment);

    // Use second handler - agent handles Multiply messages
    let multiply = Multiply(3);
    let result = context.handle_with(multiply);
    assert_eq!(result, 18); // (1 + 5) * 3 = 18

    // Verify final state
    context.with(|agent| {
      assert_eq!(agent.state, 18);
    });
  }
}
