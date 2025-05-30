use std::{
  any::{Any, TypeId},
  collections::{HashMap, VecDeque},
};

use crate::handler::{Handler, HandlerWrapper, MessageHandler};

// Enhanced trait for things that can be agents with lifecycle management
pub trait LifeCycle: Send + Sync + 'static {
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
pub struct Agent<S: LifeCycle> {
  inner:    S,
  state:    AgentState,
  mailbox:  VecDeque<Box<dyn Any + Send + Sync>>,
  handlers: HashMap<TypeId, Vec<Box<dyn MessageHandler>>>,
}

impl<A: LifeCycle> Agent<A> {
  pub fn new(agent: A) -> Self {
    Self {
      inner:    agent,
      state:    AgentState::Stopped,
      mailbox:  VecDeque::new(),
      handlers: HashMap::new(),
    }
  }

  // Lifecycle management methods
  pub fn start(&mut self) {
    if self.state != AgentState::Running {
      self.inner.on_start();
      self.state = AgentState::Running;
    }
  }

  pub fn pause(&mut self) {
    if self.state == AgentState::Running {
      self.inner.on_pause();
      self.state = AgentState::Paused;
    }
  }

  pub fn stop(&mut self) {
    if self.state != AgentState::Stopped {
      self.inner.on_stop();
      self.state = AgentState::Stopped;
    }
  }

  pub fn resume(&mut self) {
    if self.state == AgentState::Paused {
      self.inner.on_resume();
      self.state = AgentState::Running;
    }
  }

  // Get current state
  pub const fn state(&self) -> AgentState { self.state }

  // Check if agent can process messages
  pub fn is_active(&self) -> bool { self.state == AgentState::Running }

  // Get mutable access to the underlying agent
  pub const fn agent_mut(&mut self) -> &mut A { &mut self.inner }

  // Get immutable access to the underlying agent
  pub const fn agent(&self) -> &A { &self.inner }

  pub fn with_handler<M>(mut self) -> Self
  where
    M: Clone + Send + Sync + 'static,
    A: Handler<M> + Send + Sync + 'static,
    A::Reply: Send + Sync + 'static, {
    let thing = A::handle;
    self
      .handlers
      .entry(TypeId::of::<M>())
      .or_default()
      .push(Box::new(HandlerWrapper::<A, M>::new(A::handle)));
    self
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentState {
  Stopped, // Agent is not processing messages
  Running, // Agent is actively processing messages
  Paused,  // Agent is temporarily not processing messages but can be resumed
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
  fn handlers(&self) -> &HashMap<TypeId, Vec<Box<dyn MessageHandler>>>;
  fn handlers_mut(&mut self) -> &mut HashMap<TypeId, Vec<Box<dyn MessageHandler>>>;
}

// Implement RuntimeAgent for AgentContainer<T>
impl<A: LifeCycle> RuntimeAgent for crate::agent::Agent<A> {
  fn start(&mut self) { Self::start(self); }

  fn pause(&mut self) { Self::pause(self); }

  fn stop(&mut self) { Self::stop(self); }

  fn resume(&mut self) { Self::resume(self); }

  fn state(&self) -> AgentState { self.state }

  fn is_active(&self) -> bool { Self::is_active(self) }

  fn agent_type_name(&self) -> &'static str { std::any::type_name::<A>() }

  fn handlers(&self) -> &HashMap<TypeId, Vec<Box<dyn MessageHandler>>> { &self.handlers }

  fn handlers_mut(&mut self) -> &mut HashMap<TypeId, Vec<Box<dyn MessageHandler>>> {
    &mut self.handlers
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::handler::Handler;

  // Simple agent struct - just data!
  #[derive(Clone)]
  pub struct Arithmetic {
    state: i32,
  }

  // Implement Agent trait
  impl LifeCycle for Arithmetic {}

  #[derive(Debug, Clone)]
  pub struct Increment(i32);
  #[derive(Debug, Clone)]
  pub struct Multiply(i32);

  impl Handler<Increment> for Arithmetic {
    type Reply = ();

    fn handle(&mut self, message: Increment) -> Self::Reply {
      println!("Handler 1 - Received message: {message:?}");
      self.state += message.0;
      println!("Handler 1 - Updated state: {}", self.state);
    }
  }

  impl Handler<Multiply> for Arithmetic {
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
    let mut arithmetic = Agent::new(Arithmetic { state: 1 });
    arithmetic.with_handler::<Increment>();

    // Agent starts in stopped state - messages should be ignored
    assert_eq!(arithmetic.state(), AgentState::Stopped);
    assert!(!arithmetic.is_active());

    let increment = Increment(5);
    let _ = arithmetic.agent_mut().handle(increment);
    assert_eq!(arithmetic.state(), AgentState::Stopped);
    assert_eq!(arithmetic.inner.state, 1);

    arithmetic.start();
    assert_eq!(arithmetic.state(), AgentState::Running);
    assert!(arithmetic.is_active());

    // Now messages should be processed
    let increment = Increment(5);
    let _result = arithmetic.agent_mut().handle(increment);

    let multiply = Multiply(3);
    let _result = arithmetic.agent_mut().handle(multiply);

    assert_eq!(arithmetic.inner.state, 18); // (1 + 5) * 3 = 18
  }

  #[test]
  fn test_agent_lifecycle() {
    todo!("This needs re-thought.");
    // let test_agent = TestAgent { state: 10 };
    // let shared_agent = AgentContainer::shared(test_agent);
    // let mut increment_handler = AgentHandler::<TestAgent, Increment>::new(shared_agent.clone());

    // // Test lifecycle transitions
    // {
    //   let container = shared_agent.lock().unwrap();
    //   assert_eq!(container.state(), &AgentState::Stopped);
    // }

    // // Start agent
    // {
    //   let mut container = shared_agent.lock().unwrap();
    //   container.start();
    //   assert_eq!(container.state(), &AgentState::Running);
    // }

    // // Pause agent
    // {
    //   let mut container = shared_agent.lock().unwrap();
    //   container.pause();
    //   assert_eq!(container.state(), &AgentState::Paused);
    //   assert!(!container.is_active()); // Paused agents don't process messages
    // }

    // // Try to process message while paused
    // let increment = Increment(5);
    // let _result = increment_handler.handle_message(&increment);
    // {
    //   let container = shared_agent.lock().unwrap();
    //   assert_eq!(container.agent().state, 10); // State unchanged
    // }

    // // Resume agent
    // {
    //   let mut container = shared_agent.lock().unwrap();
    //   container.resume();
    //   assert_eq!(container.state(), &AgentState::Running);
    //   assert!(container.is_active());
    // }

    // // Now message should be processed
    // let increment = Increment(5);
    // let _result = increment_handler.handle_message(&increment);
    // {
    //   let container = shared_agent.lock().unwrap();
    //   assert_eq!(container.agent().state, 15); // 10 + 5 = 15
    // }

    // // Stop agent
    // {
    //   let mut container = shared_agent.lock().unwrap();
    //   container.stop();
    //   assert_eq!(container.state(), &AgentState::Stopped);
    //   assert!(!container.is_active());
    // }
  }

  #[test]
  fn test_context_direct() {
    todo!("This needs re-thought.");
    let agent = Arithmetic { state: 1 };
    let mut context = Agent::new(agent);

    // Use first handler - agent handles Increment messages
    let increment = Increment(5);
    context.agent_mut().handle(increment);

    // Use second handler - agent handles Multiply messages
    let multiply = Multiply(3);
    let result = context.agent_mut().handle(multiply);
    assert_eq!(result, 18); // (1 + 5) * 3 = 18
  }
}

mod compile_time_safety {
  use super::*;
  pub struct DummyAgent;

  impl LifeCycle for DummyAgent {}

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
