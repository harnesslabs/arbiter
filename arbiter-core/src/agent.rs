use std::{
  any::{Any, TypeId},
  collections::{HashMap, HashSet},
  sync::{atomic::AtomicBool, Arc},
  thread::JoinHandle,
};

use crate::{
  handler::{create_handler, Handler, Message, MessageHandlerFn, PackageMessage, UnpackageMessage},
  transport::{memory::InMemoryTransport, Transport},
};

// TODO: We should probably add a pause back in, but let's simplify this for now.
pub trait LifeCycle: Send + Sync + 'static {
  fn on_start(&mut self) {}

  fn on_stop(&mut self) {}
}

// TODO: I need to rename all of this stuff.  It's confusing.
// TODO: Need to make the tx and rx relative to the transport
pub struct Agent<L: LifeCycle, T: Transport> {
  address:       T::Address,
  name:          Option<String>,
  inner:         L,
  processing:    Arc<AtomicBool>,
  rx:            flume::Receiver<T::Payload>,
  tx:            flume::Sender<T::Payload>,
  handlers:      HashMap<TypeId, MessageHandlerFn<T>>,
  // TODO: We need to register deserializers in the case where the transport uses serialization
  deserializers: HashMap<TypeId, fn(T::Payload) -> T::Payload>,
}

pub struct CommunicationChannel<T: Transport> {
  tx:      flume::Sender<T::Payload>,
  address: T::Address,
  state:   Arc<AtomicBool>,
  // TODO: Hashset faster than Vec for lookups?
  types:   HashSet<TypeId>,
}

impl<T: Transport> CommunicationChannel<T> {
  pub fn send(&self, message: T::Payload) {
    println!("Attempting to send message type: {:?}", (*message).type_id());
    let state = self.state.load(std::sync::atomic::Ordering::SeqCst);
    if state && self.types.contains(&(*message).type_id()) {
      println!("Sending message type: {:?}", (*message).type_id());
      self.tx.send(message).unwrap();
    }
  }

  pub fn stop(&self) { self.state.store(false, std::sync::atomic::Ordering::SeqCst); }
}

impl<L: LifeCycle> Agent<L, InMemoryTransport> {
  pub fn new(agent: L) -> Self {
    let (tx, rx) = flume::unbounded();
    Self {
      address: AgentIdentity::generate(),
      name: None,
      inner: agent,
      processing: Arc::new(AtomicBool::new(false)),
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
      processing: Arc::new(AtomicBool::new(false)),
      rx,
      tx,
      handlers: HashMap::new(),
      deserializers: HashMap::new(),
    }
  }

  pub fn processing(&self) -> bool { self.processing.load(std::sync::atomic::Ordering::SeqCst) }
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
      state:   self.processing.clone(),
      types:   self.handlers.keys().copied().collect(),
    }
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

  pub fn process(mut self) -> JoinHandle<Self> {
    self.processing.store(true, std::sync::atomic::Ordering::SeqCst);
    std::thread::spawn(move || {
      println!("Processing pending messages for agent: {}", self.address);

      while self.processing.load(std::sync::atomic::Ordering::SeqCst) {
        if let Ok(message) = self.rx.try_recv() {
          let concrete_message_type_id = (*message).type_id();

          if let Some(handler) = self.handlers.get(&concrete_message_type_id) {
            let agent_ref: &mut dyn Any = &mut self.inner;
            let reply = handler(agent_ref, message);
            // TODO: Send reply to fabric
          }
        }
      }

      self
    })
  }
}

pub enum State<L: LifeCycle, T: Transport> {
  Running(JoinHandle<Agent<L, T>>),
  Stopped(Agent<L, T>),
}

impl<L: LifeCycle, T: Transport> State<L, T> {
  pub fn start(self) -> Self {
    if let State::Stopped(agent) = self {
      let handle = agent.process();
      State::Running(handle)
    } else {
      panic!("Agent is already running");
    }
  }

  pub fn stop(self) -> Self {
    if let State::Running(handle) = self {
      let agent = handle.join().unwrap();
      return State::Stopped(agent);
    }
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
}

// Trait for runtime-manageable agents
// TODO: For async, some of these need to return futures
pub trait RuntimeAgent<T: Transport>: Send + Sync {
  fn address(&self) -> T::Address;
  fn name(&self) -> Option<&str>;
  fn is_active(&self) -> bool;
  // fn process(self: Box<Self>) -> JoinHandle<Box<Self>>;
  fn handlers(&self) -> &HashMap<TypeId, MessageHandlerFn<T>>;

  #[cfg(test)]
  fn inner_as_any(&self) -> &dyn Any;
}

impl<L: LifeCycle, T: Transport> RuntimeAgent<T> for Agent<L, T> {
  fn address(&self) -> T::Address { self.address }

  fn name(&self) -> Option<&str> { self.name.as_deref() }

  fn is_active(&self) -> bool { self.processing.load(std::sync::atomic::Ordering::SeqCst) }

  fn handlers(&self) -> &HashMap<TypeId, MessageHandlerFn<T>> { &self.handlers }

  #[cfg(test)]
  fn inner_as_any(&self) -> &dyn Any { &self.inner }
}

#[cfg(test)]
mod tests {

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
    let agent = Agent::<Arithmetic, InMemoryTransport>::new(arithmetic);
    // Don't test specific ID values as they depend on global counter state
    assert!(agent.address().as_bytes()[0] > 0);

    assert_eq!(agent.processing(), false);

    // Start agent
    let channel = agent.communication_channel();
    let handle = agent.process();

    // Stop agent
    channel.stop();
    let agent = handle.join().unwrap();
    assert_eq!(agent.inner().state, 10);
  }

  #[test]
  fn test_single_agent_handler() {
    let arithmetic = Agent::<Arithmetic, InMemoryTransport>::new(Arithmetic { state: 1 })
      .with_handler::<Increment>();
    let channel = arithmetic.communication_channel();
    let handle = arithmetic.process();
    channel.send(Arc::new(Increment(5)));
    std::thread::sleep(std::time::Duration::from_millis(100));
    channel.stop();
    let arithmetic = handle.join().unwrap();
    assert_eq!(arithmetic.inner().state, 6);
  }

  #[test]
  fn test_multiple_agent_handlers() {
    // Create simple agent
    let mut arithmetic = Agent::<Arithmetic, InMemoryTransport>::new(Arithmetic { state: 1 });
    arithmetic = arithmetic.with_handler::<Increment>().with_handler::<Multiply>();

    // Agent starts in stopped state - messages should be ignored
    assert_eq!(arithmetic.processing(), false);
    assert!(!arithmetic.is_active());

    let channel = arithmetic.communication_channel();
    let handle = arithmetic.process();
    channel.send(Arc::new(Increment(5)));
    channel.send(Arc::new(Multiply(3)));
    std::thread::sleep(std::time::Duration::from_millis(100));
    channel.stop();
    let arithmetic = handle.join().unwrap();
    assert_eq!(arithmetic.inner().state, 18); // (1 + 5) * 3 = 18
  }
}
