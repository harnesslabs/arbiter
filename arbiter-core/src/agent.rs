use std::{
  any::{Any, TypeId},
  collections::{HashMap, VecDeque},
  marker::PhantomData,
};

use crate::{
  handler::{create_handler, Handler, Message, MessageHandlerFn, PackageMessage, UnpackageMessage},
  transport::{memory::InMemoryTransport, Runtime, SyncRuntime, Transport},
};

pub trait LifeCycle: 'static {
  fn on_start(&mut self) {}

  fn on_pause(&mut self) {}

  fn on_stop(&mut self) {}

  fn on_resume(&mut self) {}
}

// TODO: I need to rename all of this stuff.  It's confusing.
pub struct Agent<S: LifeCycle, T: Transport<R>, R: Runtime> {
  address:              T::Address,
  name:                 Option<String>,
  inner:                S,
  state:                AgentState,
  mailbox:              VecDeque<T::Payload>,
  handlers:             HashMap<TypeId, MessageHandlerFn<T, R>>,
  has_pending_messages: bool,
  _runtime:             PhantomData<R>,
}

impl<A: LifeCycle> Agent<A, InMemoryTransport, SyncRuntime> {
  pub fn new(agent: A) -> Self {
    Self {
      address:              AgentIdentity::generate(),
      name:                 None,
      inner:                agent,
      state:                AgentState::Stopped,
      mailbox:              VecDeque::new(),
      handlers:             HashMap::new(),
      has_pending_messages: false,
      _runtime:             PhantomData,
    }
  }

  /// Create an agent with a specific name
  pub fn with_name(agent: A, name: impl Into<String>) -> Self {
    Self {
      address:              AgentIdentity::generate(),
      name:                 Some(name.into()),
      inner:                agent,
      state:                AgentState::Stopped,
      mailbox:              VecDeque::new(),
      handlers:             HashMap::new(),
      has_pending_messages: false,
      _runtime:             PhantomData,
    }
  }
}

impl<A: LifeCycle, T: Transport<R>, R: Runtime> Agent<A, T, R>
where T::Payload: std::fmt::Debug
{
  /// Set the agent's name
  pub fn set_name(&mut self, name: impl Into<String>) { self.name = Some(name.into()); }

  /// Clear the agent's name
  pub fn clear_name(&mut self) { self.name = None; }

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
      self.has_pending_messages = false;
    }
  }

  pub fn resume(&mut self) {
    if self.state == AgentState::Paused {
      self.inner.on_resume();
      self.state = AgentState::Running;
      // Process any messages that were queued while paused
      self.process_pending_messages();
    }
  }

  // Get current state
  pub const fn state(&self) -> AgentState { self.state }

  // Check if agent can process messages
  pub fn is_active(&self) -> bool { self.state == AgentState::Running }

  // Get mutable access to the underlying agent
  pub const fn inner_mut(&mut self) -> &mut A { &mut self.inner }

  // Get immutable access to the underlying agent
  pub const fn inner(&self) -> &A { &self.inner }

  pub fn with_handler<M>(mut self) -> Self
  where
    M: Message,
    A: Handler<M>,
    T::Payload: UnpackageMessage<M> + PackageMessage<A::Reply>, {
    println!("Adding handler for message type: {:?}", TypeId::of::<M>());
    self.handlers.insert(TypeId::of::<M>(), create_handler::<A, M, T, R>());
    self
  }
}

/// Unique identifier for agents across fabrics
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AgentIdentity([u8; 32]);

impl AgentIdentity {
  /// Generate a cryptographically secure agent identity
  pub fn generate() -> Self {
    // For now, use a simple counter - can be upgraded to crypto later
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(1);

    let mut bytes = [0u8; 32];
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    bytes[..8].copy_from_slice(&id.to_le_bytes());

    Self(bytes)
  }

  pub fn from_bytes(bytes: [u8; 32]) -> Self { Self(bytes) }

  pub fn as_bytes(&self) -> &[u8; 32] { &self.0 }
}

impl std::fmt::Display for AgentIdentity {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let short = &self.0[..4];
    write!(f, "agent-{:02x}{:02x}{:02x}{:02x}", short[0], short[1], short[2], short[3])
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentState {
  Stopped,
  Running,
  Paused,
}

// Trait for runtime-manageable agents
// TODO: For async, some of these need to return futures
pub trait RuntimeAgent<T: Transport<R>, R: Runtime> {
  fn address(&self) -> T::Address;
  fn name(&self) -> Option<&str>;
  fn start(&mut self);
  fn pause(&mut self);
  fn stop(&mut self);
  fn resume(&mut self);
  fn state(&self) -> AgentState;
  fn is_active(&self) -> bool;
  fn should_process_mailbox(&self) -> bool;
  fn queue_message(&mut self, message: T::Payload);
  fn process_pending_messages(&mut self) -> Vec<T::Payload>;
  fn handlers(&self) -> &HashMap<TypeId, MessageHandlerFn<T, R>>;

  #[cfg(test)]
  fn inner_as_any(&self) -> &dyn Any;
}

impl<A: LifeCycle, T: Transport<R>, R: Runtime> RuntimeAgent<T, R> for crate::agent::Agent<A, T, R>
where T::Payload: std::fmt::Debug
{
  fn address(&self) -> T::Address { self.address }

  fn name(&self) -> Option<&str> { self.name.as_deref() }

  fn start(&mut self) { Self::start(self); }

  fn pause(&mut self) { Self::pause(self); }

  fn stop(&mut self) { Self::stop(self); }

  fn resume(&mut self) { Self::resume(self); }

  fn state(&self) -> AgentState { self.state }

  fn is_active(&self) -> bool { Self::is_active(self) }

  fn should_process_mailbox(&self) -> bool { self.has_pending_messages && self.is_active() }

  fn queue_message(&mut self, message: T::Payload) {
    match self.state {
      AgentState::Running | AgentState::Paused => {
        self.mailbox.push_back(message);
        self.has_pending_messages = true;
      },
      AgentState::Stopped => {
        // Stopped agents don't accept messages
      },
    }
  }

  fn process_pending_messages(&mut self) -> Vec<T::Payload> {
    let mut replies = Vec::new();

    if !self.is_active() {
      return replies; // Only process if agent is running
    }

    while let Some(message) = self.mailbox.pop_front() {
      dbg!(&message);

      let concrete_message_type_id = message.as_ref().type_id();
      println!("Message type ID for handler lookup: {:?}", concrete_message_type_id);

      if let Some(handler) = self.handlers.get(&concrete_message_type_id) {
        let agent_ref: &mut dyn Any = &mut self.inner;
        let reply = handler(agent_ref, message);
        replies.push(reply);
      } else {
        println!("Warning: No handler found for message type ID: {:?}", concrete_message_type_id);
      }
    }

    self.has_pending_messages = false; // Clear flag after processing
    replies
  }

  fn handlers(&self) -> &HashMap<TypeId, MessageHandlerFn<T, R>> { &self.handlers }

  #[cfg(test)]
  fn inner_as_any(&self) -> &dyn Any { &self.inner }
}

#[cfg(test)]
mod tests {
  use std::rc::Rc;

  use super::*;
  use crate::{
    handler::Handler,
    transport::{memory::InMemoryTransport, SyncRuntime},
  };

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

    fn handle(&mut self, message: &Increment) -> Self::Reply {
      println!("Handler 1 - Received message: {message:?}");
      self.state += message.0;
      println!("Handler 1 - Updated state: {}", self.state);
    }
  }

  impl Handler<Multiply> for Arithmetic {
    type Reply = i32;

    fn handle(&mut self, message: &Multiply) -> Self::Reply {
      println!("Handler 2 - Multiplying state {} by {}", self.state, message.0);
      self.state *= message.0;
      println!("Handler 2 - Updated state: {}", self.state);
      self.state
    }
  }

  #[test]
  fn test_agent_lifecycle() {
    let arithmetic = Arithmetic { state: 10 };
    let mut shared_agent = Agent::<Arithmetic, InMemoryTransport, SyncRuntime>::new(arithmetic);
    // Don't test specific ID values as they depend on global counter state
    assert!(shared_agent.address().as_bytes()[0] > 0);

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
    let mut arithmetic =
      Agent::<Arithmetic, InMemoryTransport, SyncRuntime>::new(Arithmetic { state: 1 })
        .with_handler::<Increment>();
    arithmetic.start();

    let increment = Increment(5);
    arithmetic.queue_message(Rc::new(increment));
    dbg!(Rc::new(Increment(5)).type_id());
    arithmetic.process_pending_messages();
    assert_eq!(arithmetic.inner.state, 6);
  }

  #[test]
  fn test_multiple_agent_handlers() {
    // Create simple agent
    let mut arithmetic =
      Agent::<Arithmetic, InMemoryTransport, SyncRuntime>::new(Arithmetic { state: 1 });
    arithmetic = arithmetic.with_handler::<Increment>().with_handler::<Multiply>();

    // Agent starts in stopped state - messages should be ignored
    assert_eq!(arithmetic.state(), AgentState::Stopped);
    assert!(!arithmetic.is_active());

    let increment = Increment(5);
    arithmetic.queue_message(Rc::new(increment));
    arithmetic.process_pending_messages();
    assert_eq!(arithmetic.state(), AgentState::Stopped);
    assert_eq!(arithmetic.inner.state, 1);

    arithmetic.start();
    assert_eq!(arithmetic.state(), AgentState::Running);
    assert!(arithmetic.is_active());

    // Now messages should be processed
    let increment = Increment(5);
    arithmetic.queue_message(Rc::new(increment));
    arithmetic.process_pending_messages();

    let multiply = Multiply(3);
    arithmetic.queue_message(Rc::new(multiply));
    arithmetic.process_pending_messages();

    assert_eq!(arithmetic.inner.state, 18); // (1 + 5) * 3 = 18
  }

  #[test]
  fn test_pause_and_resume() {
    let mut arithmetic =
      Agent::<Arithmetic, InMemoryTransport, SyncRuntime>::new(Arithmetic { state: 1 });
    arithmetic = arithmetic.with_handler::<Increment>().with_handler::<Multiply>();

    arithmetic.start();
    assert_eq!(arithmetic.state(), AgentState::Running);

    // Send a message while running - should be processed immediately via mailbox
    arithmetic.queue_message(Rc::new(Increment(5)));
    assert!(arithmetic.should_process_mailbox());
    let _ = arithmetic.process_pending_messages();
    assert_eq!(arithmetic.inner.state, 6); // 1 + 5 = 6
    assert!(!arithmetic.should_process_mailbox());

    // Pause the agent
    arithmetic.pause();
    assert_eq!(arithmetic.state(), AgentState::Paused);
    assert!(!arithmetic.is_active());

    // Send messages while paused - should be queued but not processed
    arithmetic.queue_message(Rc::new(Increment(10)));
    arithmetic.queue_message(Rc::new(Multiply(2)));

    // Agent is paused, so should_process_mailbox should be false
    assert!(!arithmetic.should_process_mailbox());
    assert_eq!(arithmetic.inner.state, 6); // State unchanged

    // Resume the agent - should process queued messages automatically
    arithmetic.resume();
    assert_eq!(arithmetic.state(), AgentState::Running);
    assert!(arithmetic.is_active());

    // Messages should have been processed during resume: (6 + 10) * 2 = 32
    assert_eq!(arithmetic.inner.state, 32);
    assert!(!arithmetic.should_process_mailbox()); // No pending messages
  }

  #[test]
  fn test_stop_and_start() {
    let mut arithmetic =
      Agent::<Arithmetic, InMemoryTransport, SyncRuntime>::new(Arithmetic { state: 1 });
    arithmetic = arithmetic.with_handler::<Increment>().with_handler::<Multiply>();

    // Start the agent
    arithmetic.start();
    assert_eq!(arithmetic.state(), AgentState::Running);

    // Send a message while running
    arithmetic.queue_message(Rc::new(Increment(5)));
    arithmetic.process_pending_messages();
    assert_eq!(arithmetic.inner.state, 6); // 1 + 5 = 6

    // Stop the agent
    arithmetic.stop();
    assert_eq!(arithmetic.state(), AgentState::Stopped);
    assert!(!arithmetic.is_active());

    // Send messages while stopped - should be ignored completely
    arithmetic.queue_message(Rc::new(Increment(100)));
    arithmetic.queue_message(Rc::new(Multiply(10)));

    // No messages should be queued since agent is stopped
    assert!(!arithmetic.should_process_mailbox());
    assert_eq!(arithmetic.inner.state, 6); // State unchanged

    // Start the agent again
    arithmetic.start();
    assert_eq!(arithmetic.state(), AgentState::Running);
    assert!(arithmetic.is_active());

    // No messages should have been queued while stopped
    assert!(!arithmetic.should_process_mailbox());
    assert_eq!(arithmetic.inner.state, 6); // State still unchanged

    // Verify agent can still receive new messages after restart
    arithmetic.queue_message(Rc::new(Increment(4)));
    arithmetic.process_pending_messages();
    assert_eq!(arithmetic.inner.state, 10); // 6 + 4 = 10
  }
}
