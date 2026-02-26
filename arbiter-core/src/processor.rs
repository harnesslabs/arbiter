use std::{any::TypeId, collections::HashMap};

use tokio::task::JoinHandle;
use tokio_stream::wrappers::UnboundedReceiverStream;

use crate::{
  agent::LifeCycle,
  handler::{Envelope, HandleResult, MessageHandlerFn},
  network::{Connection, Network, memory::InMemory},
};

pub struct Processing<T, L: LifeCycle, N: Network> {
  pub name: Option<String>,
  pub address: N::Address,
  pub(crate) task: JoinHandle<T>,
  pub(crate) outer_controller: OuterController<L>,
}

// TODO: Handle errors properly in here as it's possible to send instructions with the channel down.
impl<T: CreateProcessor<L>, N: Network, L: LifeCycle> Processing<T, L, N> {
  pub fn name(&self) -> Option<&str> {
    self.name.as_deref()
  }

  pub const fn address(&self) -> N::Address {
    self.address
  }

  pub async fn state(&mut self) -> State {
    self.outer_controller.instruction_sender.send(ControlSignal::GetState).await.unwrap();
    self.outer_controller.state_receiver.recv().await.unwrap()
  }

  pub async fn start(&mut self) {
    self.outer_controller.instruction_sender.send(ControlSignal::Start).await.unwrap();
    let state = self.outer_controller.state_receiver.recv().await.unwrap();
    assert_eq!(state, State::Running);
  }

  pub async fn stop(&mut self) {
    self.outer_controller.instruction_sender.send(ControlSignal::Stop).await.unwrap();
    let state = self.outer_controller.state_receiver.recv().await.unwrap();
    assert_eq!(state, State::Stopped);
  }

  pub async fn join(self) -> T {
    self.task.await.unwrap()
  }

  // TODO: Calling stream twice would break things, so this needs fixed.
  pub async fn stream(&mut self) -> UnboundedReceiverStream<L::Snapshot> {
    let recv = self.outer_controller.snapshot_receiver.take().unwrap();
    tokio_stream::wrappers::UnboundedReceiverStream::new(recv)
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

pub struct InnerController<L: LifeCycle> {
  pub(crate) instruction_receiver: tokio::sync::mpsc::Receiver<ControlSignal>,
  pub(crate) state_sender: tokio::sync::mpsc::Sender<State>,
  pub(crate) snapshot_sender: tokio::sync::mpsc::UnboundedSender<L::Snapshot>,
}

pub struct OuterController<L: LifeCycle> {
  pub(crate) instruction_sender: tokio::sync::mpsc::Sender<ControlSignal>,
  pub(crate) state_receiver: tokio::sync::mpsc::Receiver<State>,
  pub(crate) snapshot_receiver: Option<tokio::sync::mpsc::UnboundedReceiver<L::Snapshot>>,
}

pub struct Controller<L: LifeCycle> {
  pub(crate) inner: InnerController<L>,
  pub(crate) outer: OuterController<L>,
}

// TODO (autoparallel): These controllers are hard-coded to use flume, we should use a more generic
// controller that can be used with any channel implementation.
impl<L: LifeCycle> Controller<L> {
  // TODO: Add a default and let new take in pareameters for different channel implementations.
  #[allow(clippy::new_without_default)]
  pub fn new() -> Self {
    let (instruction_sender, instruction_receiver) = tokio::sync::mpsc::channel(8);
    let (state_sender, state_receiver) = tokio::sync::mpsc::channel(8);
    let (snapshot_sender, snapshot_receiver) = tokio::sync::mpsc::unbounded_channel();
    Self {
      inner: InnerController { instruction_receiver, state_sender, snapshot_sender },
      outer: OuterController {
        instruction_sender,
        state_receiver,
        snapshot_receiver: Some(snapshot_receiver),
      },
    }
  }
}

pub trait CreateProcessor<L: LifeCycle>: Send + Sync + Sized + 'static {
  fn name(&self) -> Option<String>;
  fn address(&self) -> <InMemory as Network>::Address;
  fn connection(&mut self) -> &mut Connection<InMemory>;
  fn handlers(&self) -> &HashMap<TypeId, MessageHandlerFn<InMemory>>;
  fn get_state(&self) -> State;
  fn set_state(&mut self, state: State);
  fn inner(&self) -> &L;
  fn inner_mut(&mut self) -> &mut L;
  fn into_inner(self) -> L;

  fn process(mut self) -> Processing<Self, L, InMemory> {
    let name = self.name();
    let address = self.address();

    let controller = Controller::new();
    let mut inner_controller = controller.inner;
    let outer_controller = controller.outer;

    // ────────────────────────────────────────────────────────────────
    // Update Observers with snapshots of the current state
    // ────────────────────────────────────────────────────────────────
    let snapshot = self.inner().snapshot();
    inner_controller.snapshot_sender.send(snapshot).unwrap();

    let task = tokio::spawn(async move {
      loop {
        // ────────────────────────────────────────────────────────────────
        // Control-plane messages (START / STOP / GET_STATE)
        // ────────────────────────────────────────────────────────────────
        let prev_state = self.get_state();
        tokio::select! {
          biased;
          control_signal = inner_controller.instruction_receiver.recv() => {
            match control_signal {
              Some(ControlSignal::Start) => {
                self.set_state(State::Running);
                inner_controller.state_sender.send(State::Running).await.unwrap();
                let start_message = self.inner_mut().on_start();
                println!("sending start_message for agent {}", self.name().as_deref().unwrap_or("unknown"));
                self.connection().network.send(Envelope::package(start_message)).await;
              },
              Some(ControlSignal::Stop) => {
                self.set_state(State::Stopped);
                inner_controller.state_sender.send(State::Stopped).await.unwrap();
                let stop_message = self.inner_mut().on_stop();
                self.connection().network.send(Envelope::package(stop_message)).await;
                break;
              },
              Some(ControlSignal::GetState) => {
                inner_controller.state_sender.send(prev_state).await.unwrap();
              },
              None => {
                break;
              },
            }
          }
          // ────────────────────────────────────────────────────────────────
          // Application messages coming from the transport
          // ────────────────────────────────────────────────────────────────
          message = self.connection().network.receive() => {
            if let Some(message) = message {
              println!("received message {:?} for agent {}", message, self.name().as_deref().unwrap_or("unknown"));
              let inner_ptr = self.inner_mut() as *mut L;
              if let Some(handler) = self.handlers().get(&message.type_id) {
                // Tell the compiler to stop annoying me
                let reply = unsafe {
                    handler(&mut *inner_ptr, message.payload)
                };

                println!("reply for agent {}", self.name().as_deref().unwrap_or("unknown"));
                // ────────────────────────────────────────────────────────────────
                // Handle the reply from the message handler and send it back over the network if needed
                // ────────────────────────────────────────────────────────────────
                match reply {
                  HandleResult::Message(message) => {
                    println!("sending reply {:?} for agent {}", message, self.name().as_deref().unwrap_or("unknown"));
                    self.connection().network.send(message).await;

                    // Update Observers with snapshots of the current state
                    let snapshot = self.inner().snapshot();
                    inner_controller.snapshot_sender.send(snapshot).unwrap();
                  },
                  HandleResult::None => {
                    // Update Observers with snapshots of the current state
                    let snapshot = self.inner().snapshot();
                    inner_controller.snapshot_sender.send(snapshot).unwrap();
                  },
                  HandleResult::Stop => {
                    self.set_state(State::Stopped);
                    inner_controller.state_sender.send(State::Stopped).await.unwrap();
                    let stop_message = self.inner_mut().on_stop();
                    self.connection().network.send(Envelope::package(stop_message)).await;
                    break;
                  },
                }
              }
            }
          }

        }
      }

      self
    });

    Processing { name, address, task, outer_controller }
  }
}
