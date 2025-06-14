use std::{any::TypeId, collections::HashMap, thread::JoinHandle};

use crate::{
  connection::{memory::InMemory, Connection, Generateable, Transport},
  handler::{
    create_handler, Envelope, HandleResult, Handler, Message, MessageHandlerFn, Package,
    Unpacackage,
  },
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

  pub fn new_with_connection(agent_inner: L, connection: &Connection<T>) -> Self {
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
    self.handlers.insert(TypeId::of::<M>(), create_handler::<M, L, T>());
    self
  }

  pub const fn address(&self) -> T::Address { self.connection.address }

  pub fn name(&self) -> Option<&str> { self.name.as_deref() }

  pub const fn get_connection(&self) -> &Connection<T> { &self.connection }
}

pub struct ProcessingAgent<L: LifeCycle, T: Transport> {
  pub name:                    Option<String>,
  pub address:                 T::Address,
  pub(crate) task:             JoinHandle<Agent<L, T>>,
  pub(crate) outer_controller: OuterController,
}

impl<L: LifeCycle, T: Transport> ProcessingAgent<L, T> {
  pub fn name(&self) -> Option<&str> { self.name.as_deref() }

  pub const fn address(&self) -> T::Address { self.address }

  pub fn state(&self) -> State {
    self.outer_controller.instruction_sender.send(ControlSignal::GetState);
    self.outer_controller.state_receiver.recv().unwrap()
  }

  pub fn start(&self) {
    self.outer_controller.instruction_sender.send(ControlSignal::Start).unwrap();
    let state = self.outer_controller.state_receiver.recv().unwrap();
    assert_eq!(state, State::Running);
  }

  pub fn stop(&self) {
    self.outer_controller.instruction_sender.send(ControlSignal::Stop).unwrap();
    let state = self.outer_controller.state_receiver.recv().unwrap();
    assert_eq!(state, State::Stopped);
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

// TODO (autoparallel): These controllers are hard-coded to use flume, we should use a more generic
// controller that can be used with any channel implementation.
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
  pub fn process(mut self) -> ProcessingAgent<L, InMemory> {
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
          inner_controller.state_sender.send(new_state);
          let start_message = self.inner.on_start();
          self.connection.transport.send(Envelope::package(start_message));
          continue;
        }

        if prev_state == State::Running && new_state == State::Stopped {
          prev_state = new_state;
          self.state = new_state;
          inner_controller.state_sender.send(new_state);
          self.inner.on_stop();
          break;
        }

        if let Some(Ok(message)) = message {
          if let Some(handler) = self.handlers.get(&message.type_id) {
            let reply = handler(&mut self.inner, message.payload);
            match reply {
              HandleResult::Message(message) => {
                self.connection.transport.send(Envelope::package(message));
              },
              HandleResult::None => {},
              HandleResult::Stop => break,
            }
          }
        }
      }

      self
    });
    ProcessingAgent { name, address, task, outer_controller }
  }
}

#[cfg(test)]
mod tests {

  use super::*;
  use crate::fixtures::*;

  #[test]
  fn test_agent_lifecycle() {
    let agent = Agent::<Logger, InMemory>::new(Logger {
      name:          "TestLogger".to_string(),
      message_count: 0,
    });
    assert_eq!(agent.state, State::Stopped);

    let processing_agent = agent.process();
    processing_agent.start();
    assert_eq!(processing_agent.state(), State::Running);

    processing_agent.stop();
    let joined_agent = processing_agent.task.join().unwrap();
    assert_eq!(joined_agent.state, State::Stopped);
  }

  #[test]
  fn test_single_agent_handler() {
    let agent = Agent::<Logger, InMemory>::new(Logger {
      name:          "TestLogger".to_string(),
      message_count: 0,
    })
    .with_handler::<TextMessage>();

    // Grab a sender from the agent
    let sender = agent.connection.transport.sender.clone();

    let processing_agent = agent.process();
    processing_agent.start();
    assert_eq!(processing_agent.state(), State::Running);

    // Send a message to the agent
    sender.send(Envelope::package(TextMessage { content: "Hello".to_string() })).unwrap();

    std::thread::sleep(std::time::Duration::from_millis(10));

    processing_agent.stop();
    let agent = processing_agent.task.join().unwrap();
    assert_eq!(agent.state, State::Stopped);
    assert_eq!(agent.inner.message_count, 1);
  }

  #[test]
  fn test_multiple_agent_handlers() {
    let mut agent = Agent::<Logger, InMemory>::new(Logger {
      name:          "TestLogger".to_string(),
      message_count: 0,
    });
    agent = agent.with_handler::<TextMessage>().with_handler::<NumberMessage>();
    let sender = agent.connection.transport.sender.clone();

    assert_eq!(agent.state, State::Stopped);

    let processing_agent = agent.process();

    processing_agent.start();
    sender.send(Envelope::package(TextMessage { content: "Hello".to_string() })).unwrap();
    sender.send(Envelope::package(NumberMessage { value: 3 })).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(10));

    processing_agent.stop();
    let agent = processing_agent.task.join().unwrap();
    assert_eq!(agent.state, State::Stopped);
    assert_eq!(agent.inner.message_count, 2);
  }
}
