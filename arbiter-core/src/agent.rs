use std::{
  any::{Any, TypeId},
  collections::HashMap,
  ops::Deref,
  sync::{Arc, Condvar, Mutex},
  thread::JoinHandle,
};

use crate::{
  connection::{memory::InMemory, Connection, Generateable, Joinable, Spawnable, Transport},
  handler::{create_handler, Envelope, Handler, Message, MessageHandlerFn, Package, Unpacackage},
};

pub struct Agent<L: LifeCycle, T: Transport> {
  pub name:   Option<String>,
  state:      State,
  inner:      L,
  connection: Connection<T>,
  handlers:   HashMap<TypeId, MessageHandlerFn<T>>,
}

impl<L: LifeCycle, T: Transport> Agent<L, T> {
  pub fn new(agent_inner: L) -> Self {
    let address = T::Address::generate();
    Self {
      name:       None,
      state:      State::Stopped,
      inner:      agent_inner,
      connection: Connection::<T>::new(address),
      handlers:   HashMap::new(),
    }
  }

  pub fn new_with_connection(agent_inner: L, connection: Connection<T>) -> Self {
    Self {
      name:       None,
      state:      State::Stopped,
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
      state:      State::Stopped,
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

  pub fn address(&self) -> T::Address { self.connection.address }

  pub fn name(&self) -> Option<&str> { self.name.as_deref() }
}

pub struct RunningAgent<L: LifeCycle, T: Transport> {
  pub name:                    Option<String>,
  pub address:                 T::Address,
  pub(crate) task:             JoinHandle<Agent<L, T>>,
  pub(crate) outer_controller: OuterController,
}

impl<L: LifeCycle, T: Transport> RunningAgent<L, T> {
  pub fn name(&self) -> Option<&str> { self.name.as_deref() }

  pub fn address(&self) -> T::Address { self.address }

  pub fn state(&self) -> State {
    self.outer_controller.instruction_sender.send(ControlSignal::GetState);
    self.outer_controller.state_receiver.recv().unwrap()
  }

  pub fn start(&self) {
    self.outer_controller.instruction_sender.send(ControlSignal::Start).unwrap();
  }

  pub fn stop(&self) {
    self.outer_controller.instruction_sender.send(ControlSignal::Stop).unwrap();
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
  Stopped,
  Running,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlSignal {
  Start,
  Stop,
  GetState,
}

pub struct InnerController {
  pub(crate) instruction_receiver: flume::Receiver<ControlSignal>,
  pub(crate) state_sender:         flume::Sender<State>,
}

pub struct OuterController {
  pub(crate) instruction_sender: flume::Sender<ControlSignal>,
  pub(crate) state_receiver:     flume::Receiver<State>,
}

pub struct Controller {
  pub(crate) inner: InnerController,
  pub(crate) outer: OuterController,
}

impl Controller {
  pub fn new() -> Self {
    let (instruction_sender, instruction_receiver) = flume::unbounded();
    let (state_sender, state_receiver) = flume::unbounded();
    Self {
      inner: InnerController { instruction_receiver, state_sender },
      outer: OuterController { instruction_sender, state_receiver },
    }
  }
}

pub trait LifeCycle: Send + Sync + 'static {
  type StartMessage: Message;
  type StopMessage: Message;
  fn on_start(&mut self) -> Self::StartMessage;
  fn on_stop(&mut self) -> Self::StopMessage;
}

impl<L: LifeCycle> Agent<L, InMemory> {
  pub fn process(mut self) -> RunningAgent<L, InMemory> {
    let name = self.name.clone();
    let address = self.address();
    let controller = Controller::new();
    let inner_controller = controller.inner;
    let outer_controller = controller.outer;

    let task = std::thread::spawn(move || {
      let mut prev_state = self.state;

      loop {
        let (new_state, message, get_state) = flume::Selector::new()
          .recv(&inner_controller.instruction_receiver, |signal| match signal {
            Ok(ControlSignal::Start) => (State::Running, None, false),
            Ok(ControlSignal::Stop) => (State::Stopped, None, false),
            Ok(ControlSignal::GetState) => (prev_state, None, true),
            Err(_) => (prev_state, None, false),
          })
          .recv(&self.connection.transport.receiver, |message| (prev_state, Some(message), false))
          .wait();

        if get_state {
          inner_controller.state_sender.send(prev_state);
          continue;
        }

        if prev_state == State::Stopped && new_state == State::Running {
          prev_state = new_state;
          self.state = new_state;
          let start_message = self.inner.on_start();
          self.connection.transport.send(Envelope::package(start_message));
          println!("Agent {}: Sent start message.", self.address());
          continue;
        }

        if prev_state == State::Running && new_state == State::Stopped {
          prev_state = new_state;
          self.state = new_state;
          self.inner.on_stop();
          println!("Agent {}: Executed on_stop. Main loop finished.", self.address());
          break;
        }

        if let Some(Ok(message)) = message {
          if let Some(handler) = self.handlers.get(&message.type_id) {
            let reply = handler(&mut self.inner, message.payload);
            self.connection.transport.send(Envelope::package(reply));
          }
        }
      }

      self
    });
    RunningAgent { name, address, task, outer_controller }
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
    assert_eq!(agent.state, State::Stopped);

    // Start agent
    let running_agent = agent.process();

    running_agent.start();
    assert_eq!(running_agent.state(), State::Running);

    running_agent.stop();
    let joined_agent = running_agent.task.join().unwrap();
    assert_eq!(joined_agent.state, State::Stopped);
  }

  #[test]
  fn test_single_agent_handler() {
    let logger = Logger { name: "TestLogger".to_string(), message_count: 0 };
    let agent = Agent::<Logger, InMemory>::new(logger).with_handler::<TextMessage>();
    let sender = agent.connection.transport.sender.clone();

    let running_agent = agent.process();
    running_agent.start();

    sender.send(Envelope::package(TextMessage { content: "Hello".to_string() })).unwrap();
    assert_eq!(running_agent.state(), State::Running);

    running_agent.stop();
    let joined_agent = running_agent.task.join().unwrap();
    assert_eq!(joined_agent.state, State::Stopped);
    assert_eq!(joined_agent.inner.message_count, 1);
  }

  #[test]
  fn test_multiple_agent_handlers() {
    let mut agent_struct = Agent::<Logger, InMemory>::new(Logger {
      name:          "TestLogger".to_string(),
      message_count: 0,
    });
    agent_struct = agent_struct.with_handler::<TextMessage>().with_handler::<NumberMessage>();
    let sender = agent_struct.connection.transport.sender.clone();

    assert_eq!(agent_struct.state, State::Stopped);

    let running_agent = agent_struct.process();

    running_agent.start();
    sender.send(Envelope::package(TextMessage { content: "Hello".to_string() })).unwrap();

    sender.send(Envelope::package(NumberMessage { value: 3 })).unwrap();

    running_agent.stop();
    let joined_agent = running_agent.task.join().unwrap();
    assert_eq!(joined_agent.state, State::Stopped);
    assert_eq!(joined_agent.inner.message_count, 2);
  }
}
