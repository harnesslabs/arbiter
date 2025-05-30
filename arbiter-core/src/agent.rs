use std::{
  any::Any,
  sync::{Arc, Mutex},
};

use crate::handler::{Handler, MessageHandler};

// Agent lifecycle states
#[derive(Debug, Clone, PartialEq, Eq)]
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

// Shared wrapper for an agent with thread-safe access
pub type SharedAgent<A> = Arc<Mutex<AgentContainer<A>>>;

// Container that manages handlers for an agent
pub struct AgentContainer<A: Agent> {
  agent: A,
  state: AgentState,
}

impl<A: Agent> AgentContainer<A> {
  pub fn new(agent: A) -> Self {
    Self {
      agent,
      state: AgentState::Stopped, // Agents start in stopped state
    }
  }

  // Create a shared version wrapped in Arc<Mutex<>>
  pub fn shared(agent: A) -> SharedAgent<A> { Arc::new(Mutex::new(Self::new(agent))) }

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

  // Get mutable access to the underlying agent
  pub fn agent_mut(&mut self) -> &mut A { &mut self.agent }

  // Get immutable access to the underlying agent
  pub fn agent(&self) -> &A { &self.agent }
}

// Handler wrapper that delegates to an agent's handler implementation
pub struct AgentHandler<A, M>
where
  A: Agent + Handler<M>,
  M: Clone + Send + Sync + 'static, {
  agent:    SharedAgent<A>,
  _phantom: std::marker::PhantomData<M>,
}

impl<A, M> AgentHandler<A, M>
where
  A: Agent + Handler<M>,
  M: Clone + Send + Sync + 'static,
{
  pub fn new(agent: SharedAgent<A>) -> Self { Self { agent, _phantom: std::marker::PhantomData } }
}

impl<A, M> MessageHandler for AgentHandler<A, M>
where
  A: Agent + Handler<M>,
  M: Clone + Send + Sync + 'static,
  A::Reply: Send + Sync + 'static,
{
  fn handle_message(&mut self, message: &dyn Any) -> Box<dyn Any + Send + Sync> {
    if let Some(typed_message) = message.downcast_ref::<M>() {
      if let Ok(mut container) = self.agent.lock() {
        // Only process if agent is active
        if container.is_active() {
          let reply = container.agent_mut().handle(typed_message.clone());
          return Box::new(reply);
        }
      }
    }
    Box::new(())
  }
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

// Wrapper for shared agents to implement RuntimeAgent
pub struct SharedAgentWrapper<A: Agent> {
  pub shared_agent: SharedAgent<A>,
}

impl<A: Agent> RuntimeAgent for SharedAgentWrapper<A> {
  fn start(&mut self) {
    if let Ok(mut container) = self.shared_agent.lock() {
      container.start();
    }
  }

  fn pause(&mut self) {
    if let Ok(mut container) = self.shared_agent.lock() {
      container.pause();
    }
  }

  fn stop(&mut self) {
    if let Ok(mut container) = self.shared_agent.lock() {
      container.stop();
    }
  }

  fn resume(&mut self) {
    if let Ok(mut container) = self.shared_agent.lock() {
      container.resume();
    }
  }

  fn state(&self) -> AgentState {
    if let Ok(container) = self.shared_agent.lock() {
      container.state().clone()
    } else {
      AgentState::Stopped
    }
  }

  fn is_active(&self) -> bool {
    if let Ok(container) = self.shared_agent.lock() {
      container.is_active()
    } else {
      false
    }
  }

  fn agent_type_name(&self) -> &'static str { std::any::type_name::<A>() }
}

// Implement RuntimeAgent for AgentContainer<T>
impl<A: Agent> RuntimeAgent for crate::agent::AgentContainer<A> {
  fn start(&mut self) { crate::agent::AgentContainer::start(self); }

  fn pause(&mut self) { crate::agent::AgentContainer::pause(self); }

  fn stop(&mut self) { crate::agent::AgentContainer::stop(self); }

  fn resume(&mut self) { crate::agent::AgentContainer::resume(self); }

  fn state(&self) -> AgentState { self.state().clone() }

  fn is_active(&self) -> bool { crate::agent::AgentContainer::is_active(self) }

  fn agent_type_name(&self) -> &'static str { std::any::type_name::<A>() }
}

// TODO (autoparallel): This could be generalized to handle different kinds of locking. In which
// case we'd make this a trait and have a default implementation that uses Arc<Mutex<A>>.
pub struct Context<A> {
  agent: Arc<Mutex<A>>,
}

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
      println!("Handler 1 - Received message: {message:?}");
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
  fn test_agent_handlers() {
    // Create simple agent
    let test_agent = TestAgent { state: 1 };
    let shared_agent = AgentContainer::shared(test_agent);

    // Create handlers for different message types
    let mut increment_handler = AgentHandler::<TestAgent, Increment>::new(shared_agent.clone());
    let mut multiply_handler = AgentHandler::<TestAgent, Multiply>::new(shared_agent.clone());

    // Agent starts in stopped state - messages should be ignored
    {
      let container = shared_agent.lock().unwrap();
      assert_eq!(container.state(), &AgentState::Stopped);
      assert!(!container.is_active());
    }

    let increment = Increment(5);
    let result = increment_handler.handle_message(&increment);
    // Should return () because agent is stopped
    {
      let container = shared_agent.lock().unwrap();
      assert_eq!(container.agent().state, 1); // State should be unchanged
    }

    // Start the agent
    {
      let mut container = shared_agent.lock().unwrap();
      container.start();
      assert_eq!(container.state(), &AgentState::Running);
      assert!(container.is_active());
    }

    // Now messages should be processed
    let increment = Increment(5);
    let _result = increment_handler.handle_message(&increment);

    let multiply = Multiply(3);
    let _result = multiply_handler.handle_message(&multiply);

    // Check final state
    {
      let container = shared_agent.lock().unwrap();
      assert_eq!(container.agent().state, 18); // (1 + 5) * 3 = 18
    }
  }

  #[test]
  fn test_agent_lifecycle() {
    let test_agent = TestAgent { state: 10 };
    let shared_agent = AgentContainer::shared(test_agent);
    let mut increment_handler = AgentHandler::<TestAgent, Increment>::new(shared_agent.clone());

    // Test lifecycle transitions
    {
      let container = shared_agent.lock().unwrap();
      assert_eq!(container.state(), &AgentState::Stopped);
    }

    // Start agent
    {
      let mut container = shared_agent.lock().unwrap();
      container.start();
      assert_eq!(container.state(), &AgentState::Running);
    }

    // Pause agent
    {
      let mut container = shared_agent.lock().unwrap();
      container.pause();
      assert_eq!(container.state(), &AgentState::Paused);
      assert!(!container.is_active()); // Paused agents don't process messages
    }

    // Try to process message while paused
    let increment = Increment(5);
    let _result = increment_handler.handle_message(&increment);
    {
      let container = shared_agent.lock().unwrap();
      assert_eq!(container.agent().state, 10); // State unchanged
    }

    // Resume agent
    {
      let mut container = shared_agent.lock().unwrap();
      container.resume();
      assert_eq!(container.state(), &AgentState::Running);
      assert!(container.is_active());
    }

    // Now message should be processed
    let increment = Increment(5);
    let _result = increment_handler.handle_message(&increment);
    {
      let container = shared_agent.lock().unwrap();
      assert_eq!(container.agent().state, 15); // 10 + 5 = 15
    }

    // Stop agent
    {
      let mut container = shared_agent.lock().unwrap();
      container.stop();
      assert_eq!(container.state(), &AgentState::Stopped);
      assert!(!container.is_active());
    }
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
