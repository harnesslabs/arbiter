//! The [`StateMachine`] trait, [`Behavior`] trait, and the [`Engine`] that runs
//! [`Behavior`]s.

use std::pin::Pin;

use super::*;
use crate::{
  environment::Database,
  messager::{MessageFrom, MessageTo},
};

/// A type alias for a pinned, boxed stream of events.
///
/// This stream is capable of handling items of any type that implements the
/// `Stream` trait, and it is both sendable across threads and synchronizable
/// between threads.
///
/// # Type Parameters
///
/// * `E`: The type of the items in the stream.
pub type EventStream<E> = Pin<Box<dyn Stream<Item = E> + Send + Sync>>;

#[derive(Debug)]
pub enum Action<DB: Database> {
  StateChange(DB::Location, DB::State),
  MessageTo(MessageTo),
}

#[derive(Debug, PartialEq, Eq)]
pub enum Event<DB: Database> {
  StateChange(DB::Location, DB::State),
  MessageFrom(MessageFrom),
}

impl<DB: Database> Clone for Event<DB>
where
  DB::Location: Clone,
  DB::State: Clone,
{
  fn clone(&self) -> Self {
    match self {
      Self::StateChange(location, state) => Self::StateChange(location.clone(), state.clone()),
      Self::MessageFrom(message) => Self::MessageFrom(message.clone()),
    }
  }
}

impl<DB: Database> Clone for Action<DB>
where
  DB::Location: Clone,
  DB::State: Clone,
{
  fn clone(&self) -> Self {
    match self {
      Self::StateChange(location, state) => Self::StateChange(location.clone(), state.clone()),
      Self::MessageTo(message) => Self::MessageTo(message.clone()),
    }
  }
}

#[derive(Clone, Default)]
pub struct Actions<DB: Database> {
  actions: Vec<Action<DB>>,
}

impl<DB: Database> Actions<DB> {
  pub fn new() -> Self { Self { actions: Vec::new() } }

  pub fn add_action(&mut self, action: Action<DB>) { self.actions.push(action); }

  pub fn into_vec(self) -> Vec<Action<DB>> { self.actions }

  pub const fn is_empty(&self) -> bool { self.actions.is_empty() }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum ControlFlow {
  Halt,

  Continue,
}

pub trait Filter<DB: Database>: Send {
  fn filter(&self, event: &Event<DB>) -> bool;
}

#[async_trait::async_trait]
pub trait Behavior<DB>: Send
where
  DB: Database + 'static,
  DB::Location: Send + Sync + 'static,
  DB::State: Send + Sync + 'static, {
  fn startup(&mut self) -> Result<(Option<Box<dyn Filter<DB>>>, Actions<DB>), ArbiterCoreError> {
    Ok((None, Actions::new()))
  }

  async fn process_event(
    &mut self,
    _event: Event<DB>,
  ) -> Result<(ControlFlow, Actions<DB>), ArbiterCoreError> {
    Ok((ControlFlow::Halt, Actions::new()))
  }
}

pub trait ConfigurableBehavior<DB: Database>: for<'de> Deserialize<'de> {
  fn create_behavior(self) -> Box<dyn Behavior<DB>>;
}
