use std::{
  any::{Any, TypeId},
  collections::HashMap,
  sync::{Arc, Condvar, Mutex},
  thread::JoinHandle,
};

use crate::{
  connection::{Connection, Transport},
  fabric::Start,
  handler::{create_handler, Envelope, Handler, Message, MessageHandlerFn, Package, Unpacackage},
};

pub struct Agent<L: LifeCycle, C: Connection> {
  pub name:              Option<String>,
  inner:                 L,
  pub(crate) controller: Arc<Controller>,
  handlers:              HashMap<TypeId, MessageHandlerFn<C>>,
  transport:             Transport<C>,
}

pub struct Controller {
  pub state:   Mutex<State>,
  pub condvar: Condvar,
}

impl Controller {
  pub fn new() -> Self { Self { state: Mutex::new(State::Stopped), condvar: Condvar::new() } }
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

// TODO: It would be worth adding a handler to each agent for an instruction for "Start"/"Stop" so
// the agent can be remotely shut off.
impl<L: LifeCycle, C: Connection> Agent<L, C> {
  pub fn new(agent_inner: L) -> Self {
    Self {
      name:       None,
      inner:      agent_inner,
      controller: Arc::new(Controller::new()),
      handlers:   HashMap::new(),
      transport:  Transport::new(),
    }
  }

  pub fn with_name(agent_inner: L, name: impl Into<String>) -> Self {
    Self {
      name:       Some(name.into()),
      inner:      agent_inner,
      controller: Arc::new(Controller::new()),
      handlers:   HashMap::new(),
      transport:  Transport::new(),
    }
  }

  pub fn set_name(&mut self, name: impl Into<String>) { self.name = Some(name.into()); }

  pub fn clear_name(&mut self) { self.name = None; }

  // Access to inner L, requires locking.
  pub fn inner_mut(&mut self) -> &mut L { &mut self.inner }

  pub fn inner(&self) -> &L { &self.inner }

  pub fn with_handler<M>(mut self) -> Self
  where
    M: Message,
    L: Handler<M>,
    C::Payload: Unpacackage<M> + Package<L::Reply>, {
    dbg!(TypeId::of::<M>());
    self.handlers.insert(TypeId::of::<M>(), create_handler::<M, L, C>());
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

pub trait RuntimeAgent<C: Connection>: Send + Sync + Any {
  fn address(&self) -> C::Address;
  fn name(&self) -> Option<&str>;
  fn transport(&self) -> &Transport<C>;
  fn add_outbound_connection(&mut self, address: C::Address, sender: C::Sender);

  // Methods to signal the agent's desired state
  fn signal_start(&self);
  fn signal_stop(&self);
  fn current_loop_state(&self) -> State;

  fn process(self) -> JoinHandle<Self>
  where Self: Sized;

  #[cfg(test)]
  fn inner_as_any(&self) -> &dyn Any;
}

impl<L: LifeCycle, C: Connection> RuntimeAgent<C> for Agent<L, C> {
  fn address(&self) -> C::Address { self.transport.inbound_connection.address() }

  fn name(&self) -> Option<&str> { self.name.as_deref() }

  fn transport(&self) -> &Transport<C> { &self.transport }

  fn add_outbound_connection(&mut self, address: C::Address, sender: C::Sender) {
    self.transport.add_outbound_connection(address, sender);
  }

  fn signal_start(&self) {
    let mut sync_guard = self.controller.state.lock().unwrap();
    if *sync_guard != State::Running {
      *sync_guard = State::Running;
      self.controller.condvar.notify_one();
    }
  }

  fn signal_stop(&self) {
    let mut sync_guard = self.controller.state.lock().unwrap();
    if *sync_guard != State::Stopped {
      *sync_guard = State::Stopped;
      self.controller.condvar.notify_one();
    }
  }

  fn current_loop_state(&self) -> State { *self.controller.state.lock().unwrap() }

  fn process(mut self) -> JoinHandle<Self> {
    std::thread::spawn(move || {
      self.inner.on_start();

      loop {
        println!("Agent {}: Loop iteration.", self.address());
        let mut state_guard = self.controller.state.lock().unwrap();

        match *state_guard {
          State::Stopped => {
            println!("Agent {}: Loop detected Stopped state. Exiting.", self.address());
            break;
          },
          State::Running => {
            // Try to receive a message. This call consumes the message from the transport if one
            // exists.
            if let Some(message) = self.transport.receive() {
              // A message was received. Release the lock and process it, then potentially more.
              drop(state_guard);

              println!(
                "Agent {}: Received message (TypeId: {:?})",
                self.address(),
                message.type_id // Corrected: Direct field access
              );
              let concrete_message_type_id = message.type_id;
              if let Some(handler) = self.handlers.get(&concrete_message_type_id) {
                let reply = handler(&mut self.inner, message.payload); // self.inner is L
                self.transport.broadcast(Envelope::package(reply));
              } else {
                println!(
                  "Agent {}: Unhandled message type {:?}",
                  self.address(),
                  concrete_message_type_id
                );
              }

              // Check for stop signal immediately after processing one message.
              // Re-acquire lock to check state.
              let mut current_state_check_guard = self.controller.state.lock().unwrap();
              if *current_state_check_guard == State::Stopped {
                println!(
                  "Agent {}: Stop signal detected after processing a message.",
                  self.address()
                );
                // The lock will be released, and the outer loop will detect the Stopped state.
              } else {
                // If not stopped, release the lock and try to process more messages in a burst.
                drop(current_state_check_guard);
                while let Some(burst_message) = self.transport.receive() {
                  println!(
                    "Agent {}: Received burst message (TypeId: {:?})",
                    self.address(),
                    burst_message.type_id // Corrected: Direct field access
                  );
                  let burst_concrete_message_type_id = burst_message.type_id;
                  if let Some(handler) = self.handlers.get(&burst_concrete_message_type_id) {
                    let _reply = handler(&mut self.inner, burst_message.payload);
                  } else {
                    println!(
                      "Agent {}: Unhandled burst message type {:?}",
                      self.address(),
                      burst_concrete_message_type_id
                    );
                  }

                  // Check for stop signal during the burst.
                  let inner_burst_state_lock = self.controller.state.lock().unwrap();
                  if *inner_burst_state_lock == State::Stopped {
                    println!(
                      "Agent {}: Stop signal detected during message burst.",
                      self.address()
                    );
                    drop(inner_burst_state_lock);
                    break; // Break from the inner burst 'while' loop
                  }
                  drop(inner_burst_state_lock);
                }
              }
              // After processing the first message and any burst,
              // the outer 'loop' will continue, re-acquire its lock, and re-check state.
            } else {
              // No message was available. Release the lock and wait for a signal.
              state_guard = Condvar::wait(&self.controller.condvar, state_guard).unwrap();
            }
          },
        }
      }

      self.inner.on_stop();
      println!("Agent {}: Executed on_stop. Main loop finished.", self.address());
      self
    })
  }

  #[cfg(test)]
  fn inner_as_any(&self) -> &dyn Any { &self.inner }
}

#[cfg(test)]
mod tests {

  use super::*; // Tests need complete rework
  use crate::{connection::memory::InMemory, fixtures::*};

  #[test]
  fn test_agent_lifecycle() {
    let logger = Logger { name: "TestLogger".to_string(), message_count: 0 };
    let agent = Agent::<Logger, InMemory>::new(logger);
    assert_eq!(agent.current_loop_state(), State::Stopped);

    // Start agent
    let controller = agent.controller.clone();
    let handle = agent.process();

    controller.signal_start();
    std::thread::sleep(std::time::Duration::from_millis(50)); // Give time for agent to start
    assert_eq!(controller.state.lock().unwrap().clone(), State::Running);

    controller.signal_stop();
    let joined_agent = handle.join().unwrap();
    assert_eq!(joined_agent.inner().message_count, 0);
    assert_eq!(joined_agent.current_loop_state(), State::Stopped);
  }

  #[test]
  fn test_single_agent_handler() {
    let logger = Logger { name: "TestLogger".to_string(), message_count: 0 };
    let agent = Agent::<Logger, InMemory>::new(logger).with_handler::<TextMessage>();
    let sender = agent.transport.inbound_connection.sender.clone();

    let controller = agent.controller.clone();
    let handle = agent.process();

    controller.signal_start();
    std::thread::sleep(std::time::Duration::from_millis(50)); // Allow agent to start and enter wait

    sender.send(Envelope::package(TextMessage { content: "Hello".to_string() })).unwrap();
    controller.condvar.notify_one(); // Wake up the agent to process the message
    std::thread::sleep(std::time::Duration::from_millis(100)); // Allow time for message
    assert_eq!(controller.state.lock().unwrap().clone(), State::Running);

    controller.signal_stop();
    let result_agent = handle.join().unwrap();

    assert_eq!(result_agent.inner().message_count, 1);
  }

  #[test]
  fn test_multiple_agent_handlers() {
    let mut agent_struct = Agent::<Logger, InMemory>::new(Logger {
      name:          "TestLogger".to_string(),
      message_count: 0,
    });
    agent_struct = agent_struct.with_handler::<TextMessage>().with_handler::<NumberMessage>();
    let sender = agent_struct.transport.inbound_connection.sender.clone();

    assert_eq!(agent_struct.current_loop_state(), State::Stopped);

    let controller = agent_struct.controller.clone();
    let handle = agent_struct.process();

    controller.signal_start();
    std::thread::sleep(std::time::Duration::from_millis(50));

    sender.send(Envelope::package(TextMessage { content: "Hello".to_string() })).unwrap();
    controller.condvar.notify_one(); // Wake up the agent to process the message
    std::thread::sleep(std::time::Duration::from_millis(50));

    sender.send(Envelope::package(NumberMessage { value: 3 })).unwrap();
    controller.condvar.notify_one(); // Wake up the agent to process the message
    std::thread::sleep(std::time::Duration::from_millis(50));

    controller.signal_stop();
    let result_agent = handle.join().unwrap();
    assert_eq!(result_agent.inner().message_count, 2);
  }
}
