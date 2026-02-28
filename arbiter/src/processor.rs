//! Execution control and observation for running actors.
//!
//! This module provides the [`Processing`] handle, which allows interacting
//! with a spawned actor's lifecycle (starting, stopping) and observing its state.

use tokio::task::JoinHandle;
use tokio_stream::wrappers::UnboundedReceiverStream;

use crate::{
  actor::LifeCycle,
  error::{ArbiterError, Result},
  network::{Network, Socket},
};

/// A handle to a running (or ready-to-run) actor task.
///
/// Provides methods to control the actor's execution state and stream its snapshots.
pub struct Processing<T, L: LifeCycle, N: Network> {
  pub name:                    Option<String>,
  pub address:                 <N::Socket as Socket>::Address,
  pub(crate) task:             JoinHandle<T>,
  pub(crate) outer_controller: OuterController<L>,
}

impl<T, N: Network, L: LifeCycle> Processing<T, L, N> {
  /// Returns the actor's configured name, if any.
  #[must_use]
  pub fn name(&self) -> Option<&str> { self.name.as_deref() }

  /// Returns the socket address assigned to this actor.
  #[must_use]
  pub const fn address(&self) -> <N::Socket as Socket>::Address { self.address }

  /// Retrieves the current execution state of the actor.
  ///
  /// # Errors
  /// Returns `ArbiterError::ChannelClosed` if the actor task has panicked or exited.
  pub async fn state(&mut self) -> Result<State> {
    self
      .outer_controller
      .instruction_sender
      .send(ControlSignal::GetState)
      .await
      .map_err(|_| ArbiterError::ChannelClosed)?;
    self.outer_controller.state_receiver.recv().await.ok_or(ArbiterError::ChannelClosed)
  }

  /// Commands the actor to transition to the `Running` state and start processing messages.
  ///
  /// # Errors
  /// Returns `ArbiterError::ChannelClosed` if the actor task is no longer running.
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

  /// Commands the actor to transition to the `Stopped` state.
  ///
  /// # Errors
  /// Returns `ArbiterError::ChannelClosed` if the actor task is no longer running.
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

  /// Awaits the completion of the actor's background task and returns its result.
  ///
  /// # Errors
  /// Returns `ArbiterError::TaskPanicked` if the background task panicked.
  pub async fn join(self) -> Result<T> { self.task.await.map_err(ArbiterError::TaskPanicked) }

  /// Retrieves a stream of the actor's snapshots. Can only be retrieved once.
  ///
  /// # Errors
  /// Returns `ArbiterError::StreamAlreadyTaken` if called more than once.
  pub fn stream(&mut self) -> Result<UnboundedReceiverStream<L::Snapshot>> {
    let recv =
      self.outer_controller.snapshot_receiver.take().ok_or(ArbiterError::StreamAlreadyTaken)?;
    Ok(tokio_stream::wrappers::UnboundedReceiverStream::new(recv))
  }
}

/// The execution state of an actor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
  /// The actor is paused or stopped and will not process messages.
  Stopped,
  /// The actor is actively routing and processing messages.
  Running,
}

/// Internal signals used to control actor execution state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlSignal {
  Start,
  Stop,
  GetState,
}

/// Internal half of the control channel, held by the actor task.
pub struct InnerController<L: LifeCycle> {
  pub(crate) instruction_receiver: tokio::sync::mpsc::Receiver<ControlSignal>,
  pub(crate) state_sender:         tokio::sync::mpsc::Sender<State>,
  pub(crate) snapshot_sender:      tokio::sync::mpsc::UnboundedSender<L::Snapshot>,
}

/// External half of the control channel, held by the `Processing` handle.
pub struct OuterController<L: LifeCycle> {
  pub(crate) instruction_sender: tokio::sync::mpsc::Sender<ControlSignal>,
  pub(crate) state_receiver:     tokio::sync::mpsc::Receiver<State>,
  pub(crate) snapshot_receiver:  Option<tokio::sync::mpsc::UnboundedReceiver<L::Snapshot>>,
}

/// A unified wrapper for both halves of the actor control channel.
pub struct Controller<L: LifeCycle> {
  pub(crate) inner: InnerController<L>,
  pub(crate) outer: OuterController<L>,
}

impl<L: LifeCycle> Controller<L> {
  /// Creates a new, connected pair of inner and outer controllers.
  #[must_use]
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
