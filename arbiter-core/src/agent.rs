use std::{
  any::TypeId,
  collections::HashMap,
  sync::{Condvar, Mutex},
  thread::JoinHandle,
};

use crate::{
  connection::{Connection, Generateable, Transport},
  handler::{create_handler, Envelope, Handler, Message, MessageHandlerFn, Package, Unpacackage},
};

pub struct Agent<L: LifeCycle, T: Transport> {
  pub name:   Option<String>,
  inner:      L,
  connection: Connection<T>,
  handlers:   HashMap<TypeId, MessageHandlerFn<T>>,
}

pub struct RunningAgent<L: LifeCycle, T: Transport> {
  pub name:        Option<String>,
  pub address:     T::Address,
  pub(crate) task: JoinHandle<Agent<L, T>>,
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
  type StartMessage: Message;
  type StopMessage: Message;
  fn on_start(&mut self) -> Self::StartMessage;
  fn on_stop(&mut self) -> Self::StopMessage;
}

// TODO: It would be worth adding a handler to each agent for an instruction for "Start"/"Stop" so
// the agent can be remotely shut off.
impl<L: LifeCycle, T: Transport> Agent<L, T> {
  pub fn new(agent_inner: L) -> Self {
    let address = T::Address::generate();
    Self {
      name:       None,
      inner:      agent_inner,
      connection: Connection::<T>::new(address),
      handlers:   HashMap::new(),
    }
  }

  pub fn new_with_connection(agent_inner: L, connection: Connection<T>) -> Self {
    Self {
      name:       None,
      inner:      agent_inner,
      connection: Connection {
        address:   T::Address::generate(),
        transport: connection.transport.join(),
      },
      handlers:   HashMap::new(),
    }
  }

  pub fn new_with_connection_and_name(
    agent_inner: L,
    connection: &Connection<T>,
    name: impl Into<String>,
  ) -> Self {
    Self {
      name:       Some(name.into()),
      inner:      agent_inner,
      connection: Connection {
        address:   connection.address,
        transport: connection.transport.join(),
      },
      handlers:   HashMap::new(),
    }
  }

  pub fn set_name(&mut self, name: impl Into<String>) { self.name = Some(name.into()); }

  pub fn clear_name(&mut self) { self.name = None; }

  pub fn with_handler<M>(mut self) -> Self
  where
    M: Message,
    L: Handler<M>,
    T::Payload: Unpacackage<M> + Package<L::Reply>, {
    dbg!(TypeId::of::<M>());
    self.handlers.insert(TypeId::of::<M>(), create_handler::<M, L, T>());
    self
  }

  fn address(&self) -> T::Address { self.connection.address }

  fn name(&self) -> Option<&str> { self.name.as_deref() }

  // fn signal_start(&self) {
  //   let mut sync_guard = self.controller.state.lock().unwrap();
  //   if *sync_guard != State::Running {
  //     *sync_guard = State::Running;
  //     self.controller.condvar.notify_one();
  //   }
  // }

  // fn signal_stop(&self) {
  //   let mut sync_guard = self.controller.state.lock().unwrap();
  //   if *sync_guard != State::Stopped {
  //     *sync_guard = State::Stopped;
  //     self.controller.condvar.notify_one();
  //   }
  // }

  fn process(mut self) -> RunningAgent<L, T> {
    let name = self.name.clone();
    let address = self.address();

    let task = std::thread::spawn(move || {
      let start_message = self.inner.on_start();
      self.transport.broadcast(Envelope::package(start_message));
      println!("Agent {}: Sent start message.", self.address());

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
    });
    RunningAgent { name, address, task }
  }
}

#[cfg(test)]
mod tests {

  use super::*; // Tests need complete rework
  use crate::{connection::memory::InMemory, fixtures::*};

  #[test]
  fn test_agent_lifecycle() {
    let logger = Logger { name: "TestLogger".to_string(), message_count: 0 };
    let agent = Agent::<Logger, InMemory>::new(logger);
    assert_eq!(*agent.controller.state.lock().unwrap(), State::Stopped);

    // Start agent
    let controller = agent.controller.clone();
    let running_agent = agent.process();

    controller.signal_start();
    std::thread::sleep(std::time::Duration::from_millis(50)); // Give time for agent to start
    assert_eq!(controller.state.lock().unwrap().clone(), State::Running);

    controller.signal_stop();
    let joined_agent = running_agent.task.join().unwrap();
    let agent = joined_agent.inner_as_any().downcast_ref::<Logger>().unwrap();
    assert_eq!(agent.message_count, 0);
    assert_eq!(*running_agent.controller.state.lock().unwrap(), State::Stopped);
  }

  #[test]
  fn test_single_agent_handler() {
    let logger = Logger { name: "TestLogger".to_string(), message_count: 0 };
    let agent = Agent::<Logger, InMemory>::new(logger).with_handler::<TextMessage>();
    let sender = agent.transport.inbound_connection.sender.clone();

    let controller = agent.controller.clone();
    let running_agent = agent.process();

    controller.signal_start();
    std::thread::sleep(std::time::Duration::from_millis(50)); // Allow agent to start and enter wait

    sender.send(Envelope::package(TextMessage { content: "Hello".to_string() })).unwrap();
    controller.condvar.notify_one(); // Wake up the agent to process the message
    std::thread::sleep(std::time::Duration::from_millis(100)); // Allow time for message
    assert_eq!(controller.state.lock().unwrap().clone(), State::Running);

    controller.signal_stop();
    let result_agent = running_agent.task.join().unwrap();
    let agent = result_agent.inner_as_any().downcast_ref::<Logger>().unwrap();
    assert_eq!(agent.message_count, 1);
  }

  #[test]
  fn test_multiple_agent_handlers() {
    let mut agent_struct = Agent::<Logger, InMemory>::new(Logger {
      name:          "TestLogger".to_string(),
      message_count: 0,
    });
    agent_struct = agent_struct.with_handler::<TextMessage>().with_handler::<NumberMessage>();
    let sender = agent_struct.transport.inbound_connection.sender.clone();

    assert_eq!(*agent_struct.controller.state.lock().unwrap(), State::Stopped);

    let controller = agent_struct.controller.clone();
    let running_agent = agent_struct.process();

    controller.signal_start();
    std::thread::sleep(std::time::Duration::from_millis(50));

    sender.send(Envelope::package(TextMessage { content: "Hello".to_string() })).unwrap();
    controller.condvar.notify_one(); // Wake up the agent to process the message
    std::thread::sleep(std::time::Duration::from_millis(50));

    sender.send(Envelope::package(NumberMessage { value: 3 })).unwrap();
    controller.condvar.notify_one(); // Wake up the agent to process the message
    std::thread::sleep(std::time::Duration::from_millis(50));

    controller.signal_stop();
    let result_agent = running_agent.task.join().unwrap();
    let agent = result_agent.inner_as_any().downcast_ref::<Logger>().unwrap();
    assert_eq!(agent.message_count, 2);
  }
}
