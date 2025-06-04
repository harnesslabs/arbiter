use std::{
  any::{Any, TypeId},
  collections::{HashMap, HashSet},
  sync::Arc,
  thread::JoinHandle,
};

use crate::{
  handler::{create_handler, Handler, Message, MessageHandlerFn, PackageMessage, UnpackageMessage},
  transport::{memory::InMemoryTransport, Transport},
};

pub trait LifeCycle: Send + Sync + 'static {
  fn on_start(&mut self) {}

  fn on_pause(&mut self) {}

  fn on_stop(&mut self) {}

  fn on_resume(&mut self) {}
}

// TODO: I need to rename all of this stuff.  It's confusing.
// TODO: Need to make the tx and rx relative to the transport
pub struct Agent<L: LifeCycle, T: Transport> {
  address:       T::Address,
  name:          Option<String>,
  inner:         L,
  state:         Arc<AtomicAgentState>,
  rx:            flume::Receiver<T::Payload>,
  tx:            flume::Sender<T::Payload>,
  handlers:      HashMap<TypeId, MessageHandlerFn<T>>,
  // TODO: We need to register deserializers in the case where the transport uses serialization
  deserializers: HashMap<TypeId, fn(T::Payload) -> T::Payload>,
}

pub struct CommunicationChannel<T: Transport> {
  tx:      flume::Sender<T::Payload>,
  address: T::Address,
  state:   Arc<AtomicAgentState>,
  // TODO: Hashset faster than Vec for lookups?
  types:   HashSet<TypeId>,
}

impl<T: Transport> CommunicationChannel<T> {
  pub fn send(&self, message: T::Payload) {
    let state = self.state.load(std::sync::atomic::Ordering::SeqCst);
    if (state == AgentState::Running || state == AgentState::Paused)
      && self.types.contains(&(*message).type_id())
    {
      self.tx.send(message).unwrap();
    }
  }

  pub fn start(&self) {
    self.state.store(AgentState::Running, std::sync::atomic::Ordering::SeqCst);
  }

  pub fn pause(&self) { self.state.store(AgentState::Paused, std::sync::atomic::Ordering::SeqCst); }

  pub fn stop(&self) { self.state.store(AgentState::Stopped, std::sync::atomic::Ordering::SeqCst); }
}

impl<L: LifeCycle> Agent<L, InMemoryTransport> {
  pub fn new(agent: L) -> Self {
    let (tx, rx) = flume::unbounded();
    Self {
      address: AgentIdentity::generate(),
      name: None,
      inner: agent,
      state: Arc::new(AtomicAgentState::new(AgentState::Stopped)),
      rx,
      tx,
      handlers: HashMap::new(),
      deserializers: HashMap::new(),
    }
  }

  /// Create an agent with a specific name
  pub fn with_name(agent: L, name: impl Into<String>) -> Self {
    let (tx, rx) = flume::unbounded();
    Self {
      address: AgentIdentity::generate(),
      name: Some(name.into()),
      inner: agent,
      state: Arc::new(AtomicAgentState::new(AgentState::Stopped)),
      rx,
      tx,
      handlers: HashMap::new(),
      deserializers: HashMap::new(),
    }
  }
}

impl<L: LifeCycle, T: Transport> Agent<L, T> {
  /// Set the agent's name
  pub fn set_name(&mut self, name: impl Into<String>) { self.name = Some(name.into()); }

  /// Clear the agent's name
  pub fn clear_name(&mut self) { self.name = None; }

  pub fn communication_channel(&self) -> CommunicationChannel<T> {
    CommunicationChannel {
      tx:      self.tx.clone(),
      address: self.address,
      state:   self.state.clone(),
      types:   self.handlers.keys().copied().collect(),
    }
  }

  // Lifecycle management methods
  pub fn start(&mut self) {
    if self.state.load(std::sync::atomic::Ordering::SeqCst) != AgentState::Running {
      self.state.store(AgentState::Running, std::sync::atomic::Ordering::SeqCst);
      self.inner.on_start();
    }
  }

  pub fn pause(&mut self) {
    if self.state.load(std::sync::atomic::Ordering::SeqCst) == AgentState::Running {
      self.state.store(AgentState::Paused, std::sync::atomic::Ordering::SeqCst);
      self.inner.on_pause();
    }
  }

  pub fn stop(&mut self) {
    if self.state.load(std::sync::atomic::Ordering::SeqCst) != AgentState::Stopped {
      self.state.store(AgentState::Stopped, std::sync::atomic::Ordering::SeqCst);
      self.inner.on_stop();
      self.rx.drain();
    }
  }

  pub fn resume(&mut self) {
    if self.state.load(std::sync::atomic::Ordering::SeqCst) == AgentState::Paused {
      self.inner.on_resume();
      self.state.store(AgentState::Running, std::sync::atomic::Ordering::SeqCst);
      self.process();
    }
  }

  // Get current state
  pub fn state(&self) -> AgentState { self.state.load(std::sync::atomic::Ordering::SeqCst) }

  // Check if agent can process messages
  pub fn is_active(&self) -> bool {
    self.state.load(std::sync::atomic::Ordering::SeqCst) == AgentState::Running
  }

  // Get mutable access to the underlying agent
  pub const fn inner_mut(&mut self) -> &mut L { &mut self.inner }

  // Get immutable access to the underlying agent
  pub const fn inner(&self) -> &L { &self.inner }

  pub fn with_handler<M>(mut self) -> Self
  where
    M: Message,
    L: Handler<M>,
    T::Payload: UnpackageMessage<M> + PackageMessage<L::Reply>, {
    println!("Adding handler for message type: {:?}", TypeId::of::<M>());
    self.handlers.insert(TypeId::of::<M>(), create_handler::<M, L, T>());
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

  pub const fn from_bytes(bytes: [u8; 32]) -> Self { Self(bytes) }

  pub const fn as_bytes(&self) -> &[u8; 32] { &self.0 }
}

impl std::fmt::Display for AgentIdentity {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let short = &self.0[..4];
    write!(f, "agent-{:02x}{:02x}{:02x}{:02x}", short[0], short[1], short[2], short[3])
  }
}

#[atomic_enum::atomic_enum]
#[derive(PartialEq, Eq)]
pub enum AgentState {
  Stopped,
  Running,
  Paused,
}

// Trait for runtime-manageable agents
// TODO: For async, some of these need to return futures
pub trait RuntimeAgent<T: Transport>: Send + Sync {
  fn address(&self) -> T::Address;
  fn name(&self) -> Option<&str>;
  fn state(&self) -> AgentState;
  fn is_active(&self) -> bool;
  fn should_process_mailbox(&self) -> bool;
  fn process(self) -> JoinHandle<Arc<dyn RuntimeAgent<T>>>;
  fn handlers(&self) -> &HashMap<TypeId, MessageHandlerFn<T>>;

  #[cfg(test)]
  fn inner_as_any(&self) -> &dyn Any;
}

impl<L: LifeCycle, T: Transport> RuntimeAgent<T> for Agent<L, T> {
  fn address(&self) -> T::Address { self.address }

  fn name(&self) -> Option<&str> { self.name.as_deref() }

  fn state(&self) -> AgentState { self.state.load(std::sync::atomic::Ordering::SeqCst) }

  fn is_active(&self) -> bool { Self::is_active(self) }

  fn should_process_mailbox(&self) -> bool { dbg!(!self.rx.is_empty()) && dbg!(self.is_active()) }

  fn process(mut self) -> JoinHandle<Arc<dyn RuntimeAgent<T>>> {
    std::thread::spawn(move || {
      println!("Processing pending messages for agent: {}", self.address);

      let drain = self.rx.drain();
      for message in drain {
        let concrete_message_type_id = (*message).type_id();

        if let Some(handler) = self.handlers.get(&concrete_message_type_id) {
          let agent_ref: &mut dyn Any = &mut self.inner;
          let reply = handler(agent_ref, message);
          todo!("Send reply to fabric");
        }
      }

      Arc::new(self) as Arc<dyn RuntimeAgent<T>>
    })
  }

  fn handlers(&self) -> &HashMap<TypeId, MessageHandlerFn<T>> { &self.handlers }

  #[cfg(test)]
  fn inner_as_any(&self) -> &dyn Any { &self.inner }
}

#[cfg(test)]
mod tests {
  use std::rc::Rc;

  use super::*;
  use crate::{handler::Handler, transport::memory::InMemoryTransport};

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
    let mut shared_agent = Agent::<Arithmetic, InMemoryTransport>::new(arithmetic);
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
    let mut arithmetic = Agent::<Arithmetic, InMemoryTransport>::new(Arithmetic { state: 1 })
      .with_handler::<Increment>();
    arithmetic.start();

    let increment = Increment(5);
    println!("Sending message to agent: {}", arithmetic.address);
    let channel = arithmetic.communication_channel();
    channel.send(Rc::new(increment));
    arithmetic.process();
    assert_eq!(arithmetic.inner.state, 6);
  }

  #[test]
  fn test_multiple_agent_handlers() {
    // Create simple agent
    let mut arithmetic = Agent::<Arithmetic, InMemoryTransport>::new(Arithmetic { state: 1 });
    arithmetic = arithmetic.with_handler::<Increment>().with_handler::<Multiply>();

    // Agent starts in stopped state - messages should be ignored
    assert_eq!(arithmetic.state(), AgentState::Stopped);
    assert!(!arithmetic.is_active());

    let increment = Increment(5);
    let channel = arithmetic.communication_channel();
    channel.send(Rc::new(increment));
    arithmetic.process();
    assert_eq!(arithmetic.state(), AgentState::Stopped);
    assert_eq!(arithmetic.inner.state, 1);

    arithmetic.start();
    assert_eq!(arithmetic.state(), AgentState::Running);
    assert!(arithmetic.is_active());

    // Now messages should be processed
    let increment = Increment(5);
    channel.send(Rc::new(increment));
    arithmetic.process();

    let multiply = Multiply(3);
    channel.send(Rc::new(multiply));
    arithmetic.process();

    assert_eq!(arithmetic.inner.state, 18); // (1 + 5) * 3 = 18
  }

  #[test]
  fn test_pause_and_resume() {
    let mut arithmetic = Agent::<Arithmetic, InMemoryTransport>::new(Arithmetic { state: 1 });
    arithmetic = arithmetic.with_handler::<Increment>().with_handler::<Multiply>();

    arithmetic.start();
    assert_eq!(arithmetic.state(), AgentState::Running);

    // Send a message while running - should be processed immediately via mailbox
    let channel = arithmetic.communication_channel();
    channel.send(Rc::new(Increment(5)));
    assert!(arithmetic.should_process_mailbox());
    let _ = arithmetic.process();
    assert_eq!(arithmetic.inner.state, 6); // 1 + 5 = 6
    assert!(!arithmetic.should_process_mailbox());

    // Pause the agent
    arithmetic.pause();
    assert_eq!(arithmetic.state(), AgentState::Paused);
    assert!(!arithmetic.is_active());

    // Send messages while paused - should be queued but not processed
    channel.send(Rc::new(Increment(10)));
    channel.send(Rc::new(Multiply(2)));

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
    let mut arithmetic = Agent::<Arithmetic, InMemoryTransport>::new(Arithmetic { state: 1 });
    arithmetic = arithmetic.with_handler::<Increment>().with_handler::<Multiply>();

    // Start the agent
    arithmetic.start();
    assert_eq!(arithmetic.state(), AgentState::Running);

    // Send a message while running
    let channel = arithmetic.communication_channel();
    channel.send(Rc::new(Increment(5)));
    arithmetic.process();
    assert_eq!(arithmetic.inner.state, 6); // 1 + 5 = 6

    // Stop the agent
    arithmetic.stop();
    assert_eq!(arithmetic.state(), AgentState::Stopped);
    assert!(!arithmetic.is_active());

    // Send messages while stopped - should be ignored completely
    channel.send(Rc::new(Increment(100)));
    channel.send(Rc::new(Multiply(10)));

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
    channel.send(Rc::new(Increment(4)));
    arithmetic.process();
    assert_eq!(arithmetic.inner.state, 10); // 6 + 4 = 10
  }
}
