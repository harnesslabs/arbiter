//! The [`StateMachine`] trait, [`Behavior`] trait, and the [`Engine`] that runs
//! [`Behavior`]s.

use std::pin::Pin;

use futures::StreamExt;

use super::*;
use crate::{
  environment::{Database, Middleware},
  messager::{Message, Messager},
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

pub enum Action<DB: Database>
where
  DB::Location: Clone,
  DB::State: Clone, {
  StateChange(DB::Location, DB::State),
  Message(Message),
}

impl<DB: Database> Clone for Action<DB>
where
  DB::Location: Clone,
  DB::State: Clone,
{
  fn clone(&self) -> Self {
    match self {
      Action::StateChange(location, state) => Action::StateChange(location.clone(), state.clone()),
      Action::Message(message) => Action::Message(message.clone()),
    }
  }
}

pub struct Actions<DB: Database> {
  actions: Vec<Action<DB>>,
}

impl<DB: Database> Actions<DB> {
  pub fn new() -> Self { Self { actions: Vec::new() } }

  pub fn add_action(&mut self, action: Action<DB>) { self.actions.push(action); }

  pub fn get_actions(&self) -> &Vec<Action<DB>> { &self.actions }
}

/// The message that is used in a [`StateMachine`] to continue or halt its
/// processing.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum ControlFlow {
  /// Used to halt the processing of a [`StateMachine`].
  Halt,

  /// Used to continue on the processing of a [`StateMachine`].
  Continue,
}

/// Filter trait that can convert from unified events to specific event types
pub trait Filter<DB: Database>: Send + Sync {
  fn filter(&self, event: Action<DB>) -> bool;
}

/// The [`Behavior`] trait is the lowest level functionality that will be used
/// by a [`StateMachine`]. This constitutes what each state transition will do.
#[async_trait::async_trait]
pub trait Behavior<DB: Database>: Send + Sync
where
  DB: Database + 'static,
  DB::Location: Send + Sync + 'static,
  DB::State: Send + Sync + 'static, {
  /// Used to start the agent.
  /// This is where the agent can engage in its specific start up activities
  /// that it can do given the current state of the world.
  /// Returns an optional filter and optional actions.
  fn startup(
    &mut self,
  ) -> Result<(Option<Box<dyn Filter<DB>>>, Option<Actions<DB>>), ArbiterCoreError>;

  /// Used to process events.
  /// This is where the agent can engage in its specific processing
  /// of events that can lead to actions being taken.
  async fn process_event(
    &mut self,
    _event: Action<DB>,
  ) -> Result<(ControlFlow, Option<Actions<DB>>), ArbiterCoreError> {
    Ok((ControlFlow::Halt, None))
  }
}

// pub struct Engine<B, E>
// where E: Send + 'static {
//   id:       String,
//   behavior: B,
//   _event:   std::marker::PhantomData<E>,
// }

// impl<B, E> Engine<B, E>
// where E: Send + 'static
// {
//   pub const fn new(behavior: B, id: String) -> Self {
//     Self { behavior, id, _event: std::marker::PhantomData }
//   }
// }

/// A trait for creating a state machine.
///
/// This trait is intended to be implemented by types that can be converted into
/// a state machine. A state machine, in this context, is an entity capable of
/// executing a set of instructions or operations based on its current state and
/// inputs it receives.
///
/// Implementers of this trait should provide the logic to initialize and return
/// a new instance of a state machine, encapsulated within a `Box<dyn
/// StateMachine>`. This allows for dynamic dispatch to the state machine's
/// methods, enabling polymorphism where different types of state machines can
/// be used interchangeably at runtime.
///
/// # Returns
///
/// - `Box<dyn StateMachine>`: A boxed state machine object that can be dynamically dispatched.
pub trait ConfigurableBehavior<DB: Database> {
  /// Creates and returns a new state machine instance.
  ///
  /// This method consumes the implementer and returns a new instance of a
  /// state machine encapsulated within a `Box<dyn StateMachine>`. The
  /// specific type of the state machine returned can vary, allowing for
  /// flexibility and reuse of the state machine logic across
  /// different contexts.
  fn create_behavior(self) -> Box<dyn Behavior<DB>>;
}

// /// A trait defining the capabilities of a state machine within the system.
// ///
// /// This trait is designed to be implemented by entities that can execute
// /// instructions based on their current state and inputs they receive. The
// /// execution of these instructions is asynchronous, allowing for non-blocking
// /// operations within the state machine's logic.
// ///
// /// Implementers of this trait must be able to be sent across threads and shared
// /// among threads safely, hence the `Send`, `Sync`, and `'static` bounds. They
// /// should also support debugging through the `Debug` trait.
// #[async_trait::async_trait]
// pub trait EngineType<DB: Database>: Send {
//   /// Executes a given instruction asynchronously.
//   ///
//   /// This method takes a mutable reference to self, allowing the state
//   /// machine to modify its state in response to the instruction. The
//   /// instruction to be executed is passed as an argument, encapsulating the
//   /// action to be performed by the state machine.
//   ///
//   /// # Parameters
//   ///
//   /// - `middleware`: The middleware for database operations.
//   /// - `messager`: The messager for communication.
//   ///
//   /// # Returns
//   ///
//   /// This method does not return a value, but it may result in state changes
//   /// within the implementing type or the generation of further instructions
//   /// or events.
//   async fn start(
//     &mut self,
//     middleware: Middleware<DB>,
//     messager: Messager,
//   ) -> Result<(), ArbiterCoreError>;
// }

// #[async_trait::async_trait]
// impl<B, E, DB> EngineType<DB> for Engine<B, E, DB>
// where
//   B: Behavior<DB, E> + Send + Sync + 'static,
//   B::Filter: Send + Sync + 'static,
//   E: Send + 'static,
//   DB: Database + 'static,
//   DB::Location: Send + Sync + 'static,
//   DB::State: Send + Sync + 'static,
// {
//   async fn start(
//     &mut self,
//     middleware: Middleware<DB>,
//     messager: Messager,
//   ) -> Result<(), ArbiterCoreError> {
//     debug!("Starting behavior, id: {}", self.id);

//     // Call startup and get optional filter and actions
//     let (filter, startup_actions) =
//       match self.behavior.startup(middleware.clone(), messager.clone()).await {
//         Ok((filter, actions)) => (filter, actions),
//         Err(e) => {
//           error!("Startup failed for behavior {:?}: \n reason: {:?}", self.id, e);
//           return Err(e);
//         },
//       };

//     debug!("Startup complete for behavior {:?}", self.id);

//     // Store the filter if provided
//     self.filter = filter;

//     // Execute startup actions if provided
//     if let Some(actions) = startup_actions {
//       self.execute_actions(actions, &middleware, &messager).await?;
//     }

//     // If we have a filter, start processing events
//     if let Some(filter) = &self.filter {
//       let unified_stream = Self::create_unified_stream(middleware.clone(), messager.clone());
//       self.process_events(unified_stream, filter, &middleware, &messager).await?;
//     } else {
//       debug!("No filter provided for behavior {:?}, not processing events", self.id);
//     }

//     Ok(())
//   }
// }

// impl<B, E, DB> Engine<B, E, DB>
// where
//   B: Behavior<DB, E> + Send + Sync + 'static,
//   B::Filter: Send + Sync + 'static,
//   E: Send + 'static,
//   DB: Database + 'static,
//   DB::Location: Send + Sync + 'static,
//   DB::State: Send + Sync + 'static,
// {
//   /// Process events from the unified stream using the filter
//   async fn process_events(
//     &mut self,
//     mut unified_stream: EventStream<UnifiedEvent<DB>>,
//     filter: &B::Filter,
//     middleware: &Middleware<DB>,
//     messager: &Messager,
//   ) -> Result<(), ArbiterCoreError> {
//     use futures::StreamExt;

//     while let Some(unified_event) = unified_stream.next().await {
//       // Use the filter to convert the unified event to the specific event type
//       if let Some(filtered_event) = filter.filter(unified_event) {
//         // Process the filtered event
//         match self.behavior.process_event(filtered_event).await? {
//           (ControlFlow::Halt, actions) => {
//             // Execute any final actions before halting
//             if let Some(actions) = actions {
//               self.execute_actions(actions, middleware, messager).await?;
//             }
//             break;
//           },
//           (ControlFlow::Continue, actions) => {
//             // Execute actions and continue processing
//             if let Some(actions) = actions {
//               self.execute_actions(actions, middleware, messager).await?;
//             }
//           },
//         }
//       }
//     }

//     Ok(())
//   }

//   /// Execute a set of actions
//   async fn execute_actions(
//     &self,
//     actions: Actions<DB>,
//     middleware: &Middleware<DB>,
//     messager: &Messager,
//   ) -> Result<(), ArbiterCoreError> {
//     for action in actions.get_actions() {
//       match action {
//         Action::StateChange((location, state)) => {
//           middleware.send(location.clone(), state.clone()).await.map_err(|e| {
//             ArbiterCoreError::EnvironmentError(format!("Failed to send state change: {:?}", e))
//           })?;
//         },
//         Action::Message(message) => {
//           messager.send(message.to.clone(), &message.data).await?;
//         },
//       }
//     }
//     Ok(())
//   }
// }
