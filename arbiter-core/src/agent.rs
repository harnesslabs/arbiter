use std::{
  any::{Any, TypeId},
  collections::HashMap,
  sync::{Arc, Mutex},
};

// Simple trait for things that can be agents
pub trait Agent: Send + Sync + 'static {}

// Container that manages handlers for an agent
pub struct AgentContainer<A: Agent> {
  agent:               A,
  registered_handlers: HashMap<TypeId, bool>, /* TODO: tracks by message type, but we may also
                                               * want to track the response type too. */
}

impl<A: Agent> AgentContainer<A> {
  pub fn new(agent: A) -> Self { Self { agent, registered_handlers: HashMap::new() } }

  // Register the agent as a handler for message type M
  pub fn register_handler<M>(&mut self)
  where
    A: crate::handler::Handler<M>,
    M: Any + Send + Sync + Clone + 'static, {
    let type_id = TypeId::of::<M>();
    self.registered_handlers.insert(type_id, true);
  }

  // Handle a message by calling the agent's handler directly
  pub fn handle_message<M>(&mut self, message: M) -> Option<Box<dyn Any>>
  where
    A: crate::handler::Handler<M>,
    M: Any + Send + Sync + Clone + 'static,
    A::Reply: 'static, {
    let type_id = TypeId::of::<M>();
    if self.registered_handlers.contains_key(&type_id) {
      let reply = self.agent.handle(message);
      Some(Box::new(reply))
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

  pub fn handle_with<M>(&self, message: M) -> <A as crate::handler::Handler<M>>::Reply
  where A: crate::handler::Handler<M> {
    let mut guard = self.agent.lock().unwrap();
    guard.handle(message)
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
    let _result = container.handle_message(multiply);

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
  }
}

mod compile_time_safety {
  use super::*;
  pub struct DummyAgent;

  impl Agent for DummyAgent {}

  /// This test demonstrates that our type system prevents invalid handler registration.
  /// The following code should NOT compile because DummyAgent doesn't implement
  /// Handler<StartProcessing>:
  ///
  /// ```compile_fail
  /// use arbiter_core::agent::{Agent, AgentContainer};
  ///
  /// pub struct DummyAgent;
  /// impl Agent for DummyAgent {}
  ///
  /// # #[derive(Debug, Clone)]
  /// # struct StartProcessing(i32);
  ///
  /// let mut dummy_agent = AgentContainer::new(DummyAgent);
  /// dummy_agent.register_handler::<StartProcessing>(); // This should fail to compile!
  /// ```
  #[allow(dead_code)]
  fn test_compile_time_safety_documentation() {
    // This test passes just by existing - the real test is in the doctest above
    println!("Compile-time safety is enforced! ✅");
  }
}
