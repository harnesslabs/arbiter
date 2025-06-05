use std::{
  any::{Any, TypeId},
  collections::HashMap,
  sync::{Arc, Condvar, Mutex},
  thread::JoinHandle,
};

use crate::{
  handler::{create_handler, Handler, Message, MessageHandlerFn, Package, Unpacackage},
  transport::{memory::InMemoryTransport, Transport},
};

pub struct Agent<L: LifeCycle, T: Transport> {
  address:     T::Address,
  name:        Option<String>,
  inner:       L,
  shared_sync: Arc<Controller>,
  handlers:    HashMap<TypeId, MessageHandlerFn<T>>,
  transport:   T,
}

pub struct Controller {
  pub state:   Mutex<State>,
  pub condvar: Condvar,
}

impl Controller {
  fn new() -> Self { Self { state: Mutex::new(State::Stopped), condvar: Condvar::new() } }
}

impl Controller {
  pub fn signal_start(&self) {
    println!("Controller: signal_start");
    let mut sync_guard = self.state.lock().unwrap();
    if *sync_guard != State::Running {
      *sync_guard = State::Running;
      self.condvar.notify_one();
    }
  }

  pub fn signal_stop(&self) {
    let mut sync_guard = self.state.lock().unwrap();
    if *sync_guard != State::Stopped {
      *sync_guard = State::Stopped;
      self.condvar.notify_one();
    }
  }
}

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum State {
  Stopped,
  Running,
}

pub trait LifeCycle: Send + Sync + 'static {
  fn on_start(&mut self) {}
  fn on_stop(&mut self) {}
}

impl<L: LifeCycle> Agent<L, InMemoryTransport> {
  pub fn new(agent_inner: L) -> Self {
    Self {
      address:     AgentIdentity::generate(),
      name:        None,
      inner:       agent_inner,
      shared_sync: Arc::new(Controller::new()),
      handlers:    HashMap::new(),
      transport:   InMemoryTransport::new(),
    }
  }

  pub fn with_name(agent_inner: L, name: impl Into<String>) -> Self {
    Self {
      address:     AgentIdentity::generate(),
      name:        Some(name.into()),
      inner:       agent_inner,
      shared_sync: Arc::new(Controller::new()),
      handlers:    HashMap::new(),
      transport:   InMemoryTransport::new(),
    }
  }

  // This method is likely not how processing status will be checked anymore.
  // It will be via RuntimeAgent::is_processing() which checks shared_sync.
  // pub fn processing(&self) -> bool { self.processing.load(std::sync::atomic::Ordering::SeqCst) }
}

impl<L: LifeCycle, T: Transport> Agent<L, T> {
  pub fn set_name(&mut self, name: impl Into<String>) { self.name = Some(name.into()); }

  pub fn clear_name(&mut self) { self.name = None; }

  pub fn controller(&self) -> Arc<Controller> { self.shared_sync.clone() }

  // Access to inner L, requires locking.
  pub fn inner_mut(&mut self) -> &mut L { &mut self.inner }

  pub fn inner(&self) -> &L { &self.inner }

  pub fn with_handler<M>(mut self) -> Self
  where
    M: Message,
    L: Handler<M>,
    T::Payload: Unpacackage<M> + Package<L::Reply>, {
    dbg!(TypeId::of::<M>());
    self.handlers.insert(TypeId::of::<M>(), create_handler::<M, L, T>());
    self
  }
}

/// Unique identifier for agents across fabrics
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AgentIdentity([u8; 32]);

impl AgentIdentity {
  pub fn generate() -> Self {
    use std::sync::atomic::{AtomicU64, Ordering}; // Keep this for unique ID generation
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

pub trait RuntimeAgent<T: Transport>: Send + Sync + Any {
  fn address(&self) -> T::Address;
  fn name(&self) -> Option<&str>;

  // Methods to signal the agent's desired state
  fn signal_start(&self);
  fn signal_stop(&self);
  fn current_loop_state(&self) -> State;

  fn process(self) -> JoinHandle<Self>
  where Self: Sized;

  #[cfg(test)]
  fn inner_as_any(&self) -> &dyn Any;
}

impl<L: LifeCycle + Send + Sync, T: Transport> RuntimeAgent<T> for Agent<L, T> {
  fn address(&self) -> T::Address { self.address }

  fn name(&self) -> Option<&str> { self.name.as_deref() }

  fn signal_start(&self) {
    let mut sync_guard = self.shared_sync.state.lock().unwrap();
    if *sync_guard != State::Running {
      *sync_guard = State::Running;
      self.shared_sync.condvar.notify_one();
    }
  }

  fn signal_stop(&self) {
    let mut sync_guard = self.shared_sync.state.lock().unwrap();
    if *sync_guard != State::Stopped {
      *sync_guard = State::Stopped;
      self.shared_sync.condvar.notify_one();
    }
  }

  fn current_loop_state(&self) -> State { *self.shared_sync.state.lock().unwrap() }

  fn process(mut self) -> JoinHandle<Self> {
    std::thread::spawn(move || {
      self.inner.on_start();

      loop {
        println!("Agent {}: Loop iteration.", self.address);
        let mut state_guard = self.shared_sync.state.lock().unwrap();

        match *state_guard {
          State::Stopped => {
            println!("Agent {}: Loop detected Stopped state. Exiting.", self.address);
            break;
          },
          State::Running => {
            if self.transport.receive().is_none() {
              state_guard = Condvar::wait(&self.shared_sync.condvar, state_guard).unwrap();
              continue;
            }

            drop(state_guard);
            let mut processed_message_this_burst = false;
            while let Some(message) = self.transport.receive() {
              processed_message_this_burst = true;
              println!(
                "Agent {}: Received message (TypeId: {:?})",
                self.address,
                message.type_id()
              );
              let concrete_message_type_id = message.type_id;
              if let Some(handler) = self.handlers.get(&concrete_message_type_id) {
                let _reply = handler(&mut self.inner, message.payload); // self.inner is L
              } else {
                println!(
                  "Agent {}: Unhandled message type {:?}",
                  self.address, concrete_message_type_id
                );
              }

              let current_state_check_guard = self.shared_sync.state.lock().unwrap();
              if *current_state_check_guard == State::Stopped {
                println!("Agent {}: Stop signal detected during message burst.", self.address);
                drop(current_state_check_guard);
                break;
              }
              drop(current_state_check_guard);
            }
          },
        }
      }

      self.inner.on_stop();
      println!("Agent {}: Executed on_stop. Main loop finished.", self.address);
      self
    })
  }

  #[cfg(test)]
  fn inner_as_any(&self) -> &dyn Any { &self.inner }
}

#[cfg(test)]
mod tests {

  use super::*; // Tests need complete rework
  use crate::{
    handler::{Envelope, Handler},
    transport::memory::InMemoryTransport,
  };

  #[derive(Clone)]
  pub struct Arithmetic {
    state: i32,
  }

  impl LifeCycle for Arithmetic {}

  #[derive(Debug, Clone)]
  pub struct Increment(i32);
  #[derive(Debug, Clone)]
  pub struct Multiply(i32);

  impl Handler<Increment> for Arithmetic {
    type Reply = ();

    fn handle(&mut self, message: &Increment) -> Self::Reply {
      println!(
        "Handler 1 (Agent {}): Received Increment({}), current state: {}",
        AgentIdentity::generate(), // FIXME
        message.0,
        self.state
      );
      self.state += message.0;
      println!(
        "Handler 1 (Agent {}): New state: {}",
        AgentIdentity::generate(), // FIXME
        self.state
      );
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
    assert_eq!(agent.current_loop_state(), State::Stopped);

    // Start agent
    let controller = agent.controller();
    let handle = agent.process();

    controller.signal_start();
    std::thread::sleep(std::time::Duration::from_millis(50)); // Give time for agent to start
    assert_eq!(controller.state.lock().unwrap().clone(), State::Running);

    controller.signal_stop();
    let joined_agent = handle.join().unwrap();
    assert_eq!(joined_agent.inner().state, 10);
    assert_eq!(joined_agent.current_loop_state(), State::Stopped);
  }

  #[test]
  fn test_single_agent_handler() {
    let arithmetic = Arithmetic { state: 1 }; // Initial state
    let agent_struct =
      Agent::<Arithmetic, InMemoryTransport>::new(arithmetic).with_handler::<Increment>();

    let sender = agent_struct.transport.sender.clone();
    let controller = agent_struct.controller();
    let handle = agent_struct.process();

    controller.signal_start();
    std::thread::sleep(std::time::Duration::from_millis(50)); // Allow agent to start and enter wait

    sender.send(Envelope::package(Increment(5))).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(100)); // Allow time for message
    assert_eq!(controller.state.lock().unwrap().clone(), State::Running);

    controller.signal_stop();
    let result_agent = handle.join().unwrap();

    assert_eq!(result_agent.inner().state, 6); // Expected state: 1 (initial) + 5 (increment) = 6
  }

  // #[test]
  // fn test_multiple_agent_handlers() {
  //   let mut agent_struct = Agent::<Arithmetic, InMemoryTransport>::new(Arithmetic { state: 1 });
  //   agent_struct = agent_struct.with_handler::<Increment>().with_handler::<Multiply>();

  //   assert_eq!(agent_struct.current_loop_state(), State::Stopped);

  //   let channel = agent_struct.controller();
  //   let handle = agent_struct.process();

  //   channel.signal_start();
  //   std::thread::sleep(std::time::Duration::from_millis(50));

  //   channel.send(Envelope::package(Increment(5)));
  //   std::thread::sleep(std::time::Duration::from_millis(50)); // process (1+5=6)

  //   channel.send(Envelope::package(Multiply(3)));
  //   std::thread::sleep(std::time::Duration::from_millis(50)); // process (6*3=18)

  //   channel.signal_stop();
  //   let result_agent = handle.join().unwrap();
  //   assert_eq!(result_agent.inner().state, 18); // (1 + 5) * 3 = 18
  // }
}
