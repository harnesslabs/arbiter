use tokio::task::JoinHandle;
use tokio_stream::wrappers::UnboundedReceiverStream;

use crate::{
  actor::LifeCycle,
  error::{ArbiterError, Result},
  network::Network,
};

pub struct Processing<T, L: LifeCycle, N: Network> {
  pub name: Option<String>,
  pub address: N::Address,
  pub(crate) task: JoinHandle<T>,
  pub(crate) outer_controller: OuterController<L>,
}

impl<T, N: Network, L: LifeCycle> Processing<T, L, N> {
  pub fn name(&self) -> Option<&str> {
    self.name.as_deref()
  }

  pub const fn address(&self) -> N::Address {
    self.address
  }

  pub async fn state(&mut self) -> Result<State> {
    self
      .outer_controller
      .instruction_sender
      .send(ControlSignal::GetState)
      .await
      .map_err(|_| ArbiterError::ChannelClosed)?;
    self.outer_controller.state_receiver.recv().await.ok_or(ArbiterError::ChannelClosed)
  }

  pub async fn start(&mut self) -> Result<()> {
    self
      .outer_controller
      .instruction_sender
      .send(ControlSignal::Start)
      .await
      .map_err(|_| ArbiterError::ChannelClosed)?;
    let state =
      self.outer_controller.state_receiver.recv().await.ok_or(ArbiterError::ChannelClosed)?;
    debug_assert_eq!(state, State::Running);
    Ok(())
  }

  pub async fn stop(&mut self) -> Result<()> {
    self
      .outer_controller
      .instruction_sender
      .send(ControlSignal::Stop)
      .await
      .map_err(|_| ArbiterError::ChannelClosed)?;
    let state =
      self.outer_controller.state_receiver.recv().await.ok_or(ArbiterError::ChannelClosed)?;
    debug_assert_eq!(state, State::Stopped);
    Ok(())
  }

  pub async fn join(self) -> Result<T> {
    self.task.await.map_err(ArbiterError::TaskPanicked)
  }

  pub fn stream(&mut self) -> Result<UnboundedReceiverStream<L::Snapshot>> {
    let recv =
      self.outer_controller.snapshot_receiver.take().ok_or(ArbiterError::StreamAlreadyTaken)?;
    Ok(tokio_stream::wrappers::UnboundedReceiverStream::new(recv))
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

impl<L: LifeCycle> Controller<L> {
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
