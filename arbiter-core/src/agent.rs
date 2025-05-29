use std::{
  any::{Any, TypeId},
  collections::HashMap,
  sync::{Arc, Mutex},
};

// Agent lifecycle states
#[derive(Debug, Clone, PartialEq)]
pub enum AgentState {
  Stopped, // Agent is not processing messages
  Running, // Agent is actively processing messages
  Paused,  // Agent is temporarily not processing messages but can be resumed
}

// Enhanced trait for things that can be agents with lifecycle management
pub trait Agent: Send + Sync + 'static {
  // Lifecycle hooks - agents can override these for custom behavior
  fn on_start(&mut self) {
    // Default implementation does nothing
  }

  fn on_pause(&mut self) {
    // Default implementation does nothing
  }

  fn on_stop(&mut self) {
    // Default implementation does nothing
  }

  fn on_resume(&mut self) {
    // Default implementation does nothing
  }
}

// Container that manages handlers for an agent
pub struct AgentContainer<A: Agent> {
  agent:               A,
  state:               AgentState,
  registered_handlers: HashMap<TypeId, bool>, /* TODO: tracks by message type, but we may also
                                               * want to track the response type too. */
}

impl<A: Agent> AgentContainer<A> {
  pub fn new(agent: A) -> Self {
    Self {
      agent,
      state: AgentState::Stopped, // Agents start in stopped state
      registered_handlers: HashMap::new(),
    }
  }

  // Lifecycle management methods
  pub fn start(&mut self) {
    if self.state != AgentState::Running {
      self.agent.on_start();
      self.state = AgentState::Running;
    }
  }

  pub fn pause(&mut self) {
    if self.state == AgentState::Running {
      self.agent.on_pause();
      self.state = AgentState::Paused;
    }
  }

  pub fn stop(&mut self) {
    if self.state != AgentState::Stopped {
      self.agent.on_stop();
      self.state = AgentState::Stopped;
    }
  }

  pub fn resume(&mut self) {
    if self.state == AgentState::Paused {
      self.agent.on_resume();
      self.state = AgentState::Running;
    }
  }

  // Get current state
  pub fn state(&self) -> &AgentState { &self.state }

  // Check if agent can process messages
  pub fn is_active(&self) -> bool { self.state == AgentState::Running }

  // Register the agent as a handler for message type M
  pub fn register_handler<M>(&mut self)
  where
    A: crate::handler::Handler<M>,
    M: Any + Send + Sync + Clone + 'static, {
    let type_id = TypeId::of::<M>();
    self.registered_handlers.insert(type_id, true);
  }

  // Handle a message by calling the agent's handler directly - only if agent is active
  pub fn handle_message<M>(&mut self, message: M) -> Option<Box<dyn Any>>
  where
    A: crate::handler::Handler<M>,
    M: Any + Send + Sync + Clone + 'static,
    A::Reply: 'static, {
    // Only process messages if agent is running
    if !self.is_active() {
      return None;
    }

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

    // Agent starts in stopped state - messages should be ignored
    assert_eq!(container.state(), &AgentState::Stopped);
    assert!(!container.is_active());

    let increment = Increment(5);
    let result = container.handle_message(increment);
    assert!(result.is_none()); // Should return None because agent is stopped
    assert_eq!(container.agent().state, 1); // State should be unchanged

    // Start the agent
    container.start();
    assert_eq!(container.state(), &AgentState::Running);
    assert!(container.is_active());

    // Now messages should be processed
    let increment = Increment(5);
    let result = container.handle_message(increment);
    assert!(result.is_some()); // Should process message now

    let multiply = Multiply(3);
    let _result = container.handle_message(multiply);

    // Check final state
    assert_eq!(container.agent().state, 18); // (1 + 5) * 3 = 18
  }

  #[test]
  fn test_agent_lifecycle() {
    let test_agent = TestAgent { state: 10 };
    let mut container = AgentContainer::new(test_agent);
    container.register_handler::<Increment>();

    // Test lifecycle transitions
    assert_eq!(container.state(), &AgentState::Stopped);

    // Start agent
    container.start();
    assert_eq!(container.state(), &AgentState::Running);

    // Pause agent
    container.pause();
    assert_eq!(container.state(), &AgentState::Paused);
    assert!(!container.is_active()); // Paused agents don't process messages

    // Try to process message while paused
    let increment = Increment(5);
    let result = container.handle_message(increment);
    assert!(result.is_none()); // Should be ignored
    assert_eq!(container.agent().state, 10); // State unchanged

    // Resume agent
    container.resume();
    assert_eq!(container.state(), &AgentState::Running);
    assert!(container.is_active());

    // Now message should be processed
    let increment = Increment(5);
    let result = container.handle_message(increment);
    assert!(result.is_some());
    assert_eq!(container.agent().state, 15); // 10 + 5 = 15

    // Stop agent
    container.stop();
    assert_eq!(container.state(), &AgentState::Stopped);
    assert!(!container.is_active());
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
