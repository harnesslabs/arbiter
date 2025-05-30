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

// TODO: I need to rename all of this stuff.  It's confusing.
pub struct Agent<S: LifeCycle> {
  inner:            S,
  state:            AgentState,
  mailbox:          VecDeque<Box<dyn Any + Send + Sync>>,
  handlers:         HashMap<TypeId, Vec<Box<dyn MessageHandler>>>,
  needs_processing: bool, // Simple flag for WASM-compatible alerting
}

impl<A: LifeCycle> Agent<A> {
  pub fn new(agent: A) -> Self {
    Self {
      inner:            agent,
      state:            AgentState::Stopped,
      mailbox:          VecDeque::new(),
      handlers:         HashMap::new(),
      needs_processing: false,
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
      // Clear the mailbox to prevent any messages from being processed
      self.mailbox.clear();
      self.needs_processing = false;
    }
  }

  pub fn resume(&mut self) {
    if self.state == AgentState::Paused {
      self.inner.on_resume();
      self.state = AgentState::Running;
      // Process any messages that were queued while paused
      self.process_mailbox();
    }
  }

  // Get current state
  pub const fn state(&self) -> AgentState { self.state }

  // Check if agent can process messages
  pub fn is_active(&self) -> bool { self.state == AgentState::Running }

  // Check if agent needs mailbox processing (WASM-compatible alert system)
  pub fn needs_processing(&self) -> bool { self.needs_processing && self.is_active() }

  // Send message to mailbox based on agent state
  pub fn send_to_mailbox<M>(&mut self, message: M)
  where M: Any + Send + Sync + 'static {
    match self.state {
      AgentState::Running | AgentState::Paused => {
        self.mailbox.push_back(Box::new(message));
        self.needs_processing = true;
      },
      AgentState::Stopped => {
        // Stopped agents don't accept messages
      },
    }
  }

  // Get mutable access to the underlying agent
  pub const fn agent_mut(&mut self) -> &mut A { &mut self.inner }

  // Get immutable access to the underlying agent
  pub const fn agent(&self) -> &A { &self.inner }

  // Process all messages in the mailbox
  pub fn process_mailbox(&mut self) -> Vec<Box<dyn Any + Send + Sync>> {
    let mut all_replies = Vec::new();

    if !self.is_active() {
      return all_replies; // Only process if agent is running
    }

    while let Some(message) = self.mailbox.pop_front() {
      let replies = self.process_any_message(&*message);
      all_replies.extend(replies);
    }

    self.needs_processing = false; // Clear flag after processing
    all_replies
  }

  // TODO: This function seems redundant.  We should just use process_any_message.
  pub fn process_message<M>(&mut self, message: M) -> Vec<Box<dyn Any + Send + Sync>>
  where M: Any + Clone + Send + Sync + 'static {
    if !self.is_active() {
      return Vec::new();
    }

    let mut replies = Vec::new();
    let type_id = TypeId::of::<M>();
    if let Some(handlers) = self.handlers.get(&type_id) {
      for handler in handlers {
        let agent_any: &mut dyn Any = &mut self.inner;
        let message_any: &dyn Any = &message;
        let reply = handler.handle_message(agent_any, message_any);
        replies.push(reply);
      }
    }
    replies
  }

  pub fn with_handler<M>(mut self) -> Self
  where
    M: Clone + Send + Sync + 'static,
    A: Handler<M> + Send + Sync + 'static,
    A::Reply: Send + Sync + 'static, {
    self
      .handlers
      .entry(TypeId::of::<M>())
      .or_default()
      .push(Box::new(HandlerWrapper::<A, M> { _phantom: std::marker::PhantomData }));
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
  fn needs_processing(&self) -> bool;
  fn send_to_mailbox(&mut self, message: Box<dyn Any + Send + Sync>);
  fn process_mailbox(&mut self) -> Vec<Box<dyn Any + Send + Sync>>;
  fn agent_type_name(&self) -> &'static str;
  fn handlers(&self) -> &HashMap<TypeId, Vec<Box<dyn MessageHandler>>>;
  fn handlers_mut(&mut self) -> &mut HashMap<TypeId, Vec<Box<dyn MessageHandler>>>;
  fn process_any_message(&mut self, message: &dyn Any) -> Vec<Box<dyn Any + Send + Sync>>;
}

impl<A: LifeCycle> RuntimeAgent for crate::agent::Agent<A> {
  fn start(&mut self) { Self::start(self); }

  fn pause(&mut self) { Self::pause(self); }

  fn stop(&mut self) { Self::stop(self); }

  fn resume(&mut self) { Self::resume(self); }

  fn state(&self) -> AgentState { self.state }

  fn is_active(&self) -> bool { Self::is_active(self) }

  fn needs_processing(&self) -> bool { Self::needs_processing(self) }

  fn send_to_mailbox(&mut self, message: Box<dyn Any + Send + Sync>) {
    Self::send_to_mailbox(self, message);
  }

  fn process_mailbox(&mut self) -> Vec<Box<dyn Any + Send + Sync>> { Self::process_mailbox(self) }

  fn agent_type_name(&self) -> &'static str { std::any::type_name::<A>() }

  fn handlers(&self) -> &HashMap<TypeId, Vec<Box<dyn MessageHandler>>> { &self.handlers }

  fn handlers_mut(&mut self) -> &mut HashMap<TypeId, Vec<Box<dyn MessageHandler>>> {
    &mut self.handlers
  }

  fn process_any_message(&mut self, message: &dyn Any) -> Vec<Box<dyn Any + Send + Sync>> {
    if !self.is_active() {
      return Vec::new();
    }

    let mut replies = Vec::new();
    let type_id = message.type_id();
    if let Some(handlers) = self.handlers.get(&type_id) {
      for handler in handlers {
        let agent_any: &mut dyn Any = &mut self.inner;
        let reply = handler.handle_message(agent_any, message);
        replies.push(reply);
      }
    }
    replies
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
  fn test_agent_lifecycle() {
    let arithmetic = Arithmetic { state: 10 };
    let mut shared_agent = Agent::new(arithmetic);

    assert_eq!(shared_agent.state(), AgentState::Stopped);

    // Start agent
    shared_agent.start();
    assert_eq!(shared_agent.state(), AgentState::Running);

    // Pause agent
    shared_agent.pause();
    assert_eq!(shared_agent.state(), AgentState::Paused);
    assert!(!shared_agent.is_active()); // Paused agents don't process messages

    // Resume agent
    shared_agent.resume();
    assert_eq!(shared_agent.state(), AgentState::Running);
    assert!(shared_agent.is_active());

    // Stop agent
    shared_agent.stop();
    assert_eq!(shared_agent.state(), AgentState::Stopped);
    assert!(!shared_agent.is_active());
  }

  #[test]
  fn test_single_agent_handler() {
    let mut arithmetic = Agent::new(Arithmetic { state: 1 }).with_handler::<Increment>();
    arithmetic.start();

    let increment = Increment(5);
    let _result = arithmetic.process_message(increment);
    assert_eq!(arithmetic.inner.state, 6);
  }

  #[test]
  fn test_multiple_agent_handlers() {
    // Create simple agent
    let mut arithmetic = Agent::new(Arithmetic { state: 1 });
    arithmetic = arithmetic.with_handler::<Increment>().with_handler::<Multiply>();

    // Agent starts in stopped state - messages should be ignored
    assert_eq!(arithmetic.state(), AgentState::Stopped);
    assert!(!arithmetic.is_active());

    let increment = Increment(5);
    arithmetic.process_message(increment);
    assert_eq!(arithmetic.state(), AgentState::Stopped);
    assert_eq!(arithmetic.inner.state, 1);

    arithmetic.start();
    assert_eq!(arithmetic.state(), AgentState::Running);
    assert!(arithmetic.is_active());

    // Now messages should be processed
    let increment = Increment(5);
    let _result = arithmetic.process_message(increment);

    let multiply = Multiply(3);
    let _result = arithmetic.process_message(multiply);

    assert_eq!(arithmetic.inner.state, 18); // (1 + 5) * 3 = 18
  }

  #[test]
  fn test_pause_and_resume() {
    let mut arithmetic = Agent::new(Arithmetic { state: 1 });
    arithmetic = arithmetic.with_handler::<Increment>().with_handler::<Multiply>();

    arithmetic.start();
    assert_eq!(arithmetic.state(), AgentState::Running);

    // Send a message while running - should be processed immediately via mailbox
    arithmetic.send_to_mailbox(Increment(5));
    assert!(arithmetic.needs_processing());
    let replies = arithmetic.process_mailbox();
    assert_eq!(arithmetic.inner.state, 6); // 1 + 5 = 6
    assert!(!arithmetic.needs_processing());

    // Pause the agent
    arithmetic.pause();
    assert_eq!(arithmetic.state(), AgentState::Paused);
    assert!(!arithmetic.is_active());

    // Send messages while paused - should be queued but not processed
    arithmetic.send_to_mailbox(Increment(10));
    arithmetic.send_to_mailbox(Multiply(2));

    // Agent is paused, so needs_processing should be false
    assert!(!arithmetic.needs_processing());
    assert_eq!(arithmetic.inner.state, 6); // State unchanged

    // Resume the agent - should process queued messages automatically
    arithmetic.resume();
    assert_eq!(arithmetic.state(), AgentState::Running);
    assert!(arithmetic.is_active());

    // Messages should have been processed during resume: (6 + 10) * 2 = 32
    assert_eq!(arithmetic.inner.state, 32);
    assert!(!arithmetic.needs_processing()); // No pending messages
  }

  #[test]
  fn test_stop_and_start() {
    let mut arithmetic = Agent::new(Arithmetic { state: 1 });
    arithmetic = arithmetic.with_handler::<Increment>().with_handler::<Multiply>();

    // Start the agent
    arithmetic.start();
    assert_eq!(arithmetic.state(), AgentState::Running);

    // Send a message while running
    arithmetic.send_to_mailbox(Increment(5));
    arithmetic.process_mailbox();
    assert_eq!(arithmetic.inner.state, 6); // 1 + 5 = 6

    // Stop the agent
    arithmetic.stop();
    assert_eq!(arithmetic.state(), AgentState::Stopped);
    assert!(!arithmetic.is_active());

    // Send messages while stopped - should be ignored completely
    arithmetic.send_to_mailbox(Increment(100));
    arithmetic.send_to_mailbox(Multiply(10));

    // No messages should be queued since agent is stopped
    assert!(!arithmetic.needs_processing());
    assert_eq!(arithmetic.inner.state, 6); // State unchanged

    // Start the agent again
    arithmetic.start();
    assert_eq!(arithmetic.state(), AgentState::Running);
    assert!(arithmetic.is_active());

    // No messages should have been queued while stopped
    assert!(!arithmetic.needs_processing());
    assert_eq!(arithmetic.inner.state, 6); // State still unchanged

    // Verify agent can still receive new messages after restart
    arithmetic.send_to_mailbox(Increment(4));
    arithmetic.process_mailbox();
    assert_eq!(arithmetic.inner.state, 10); // 6 + 4 = 10
  }
}
