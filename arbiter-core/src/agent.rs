use super::*;
use crate::{
  environment::{Database, Middleware},
  machine::{Action, Behavior, Event, EventStream, Filter},
  messager::{Message, MessageFrom},
};

/// An agent is an entity capable of processing events and producing actions.
/// These are the core actors in simulations or in onchain systems.
/// Agents can be connected of other agents either as a dependent, or a
/// dependency.
///
/// # How it works
/// When the [`World`] that owns the [`Agent`] is ran, it has each [`Agent`] run
/// each of its [`Behavior`]s `startup()` methods. The [`Behavior`]s themselves
/// will return a stream of events that then let the [`Behavior`] move into the
/// `State::Processing` stage.
// TODO: Having to have `Debug` here is annoying
pub struct Agent<DB: Database> {
  pub id: String,

  pub stream: Stream<DB>,

  pub sender: Sender<DB>,

  pub(crate) behaviors: Vec<Box<dyn Behavior<DB>>>,
}

pub struct Stream<DB: Database> {
  stream:  EventStream<Event<DB>>,
  filters: HashMap<String, Box<dyn Filter<DB>>>,
}

impl<DB: Database> Stream<DB> {
  /// Add a filter to the stream with a given identifier
  pub fn add_filter(&mut self, id: String, filter: Box<dyn Filter<DB>>) {
    self.filters.insert(id, filter);
  }

  /// Get a reference to the filters
  pub fn filters(&self) -> &HashMap<String, Box<dyn Filter<DB>>> { &self.filters }

  /// Get a mutable reference to the event stream
  pub fn stream_mut(&mut self) -> &mut EventStream<Event<DB>> { &mut self.stream }
}

pub struct Sender<DB: Database>
where
  DB::Location: Clone,
  DB::State: Clone, {
  state_change_sender: mpsc::Sender<(DB::Location, DB::State)>,
  message_sender:      broadcast::Sender<Message>,
}

impl<DB: Database> Clone for Sender<DB>
where
  DB::Location: Clone,
  DB::State: Clone,
{
  fn clone(&self) -> Self {
    Self {
      state_change_sender: self.state_change_sender.clone(),
      message_sender:      self.message_sender.clone(),
    }
  }
}

impl<DB: Database> Agent<DB> {
  /// Creates a new [`AgentBuilder`] instance with a specified identifier.
  ///
  /// This method initializes an [`AgentBuilder`] with the provided `id` and
  /// sets the `behavior_engines` field to `None`. The returned
  /// [`AgentBuilder`] can be further configured using its methods before
  /// finalizing the creation of an [`Agent`].
  ///
  /// # Arguments
  ///
  /// * `id` - A string slice that holds the identifier for the agent being built.
  ///
  /// # Returns
  ///
  /// Returns an [`AgentBuilder`] instance that can be used to configure and
  /// build an [`Agent`].
  pub fn builder(id: &str) -> AgentBuilder<DB> {
    AgentBuilder { id: id.to_owned(), behaviors: Vec::new() }
  }

  /// Execute a list of actions
  pub async fn execute_actions(
    &self,
    actions: crate::machine::Actions<DB>,
  ) -> Result<(), ArbiterCoreError> {
    for action in actions.into_vec() {
      match action {
        Action::StateChange(location, state) => {
          if let Err(e) = self.sender.state_change_sender.send((location, state)).await {
            return Err(ArbiterCoreError::DatabaseError(format!(
              "Failed to send state change: {:?}",
              e
            )));
          }
        },
        // TODO: We should automatically serialize here, but not doing it for now.
        Action::MessageTo(message) =>
          if let Err(e) = self.sender.message_sender.send(Message {
            from: self.id.clone(),
            to:   message.to,
            data: message.data,
          }) {
            return Err(ArbiterCoreError::MessagerError(format!(
              "Failed to send message: {:?}",
              e
            )));
          },
      }
    }
    Ok(())
  }
}

/// [`AgentBuilder`] represents the intermediate state of agent creation before
/// it is converted into a full on [`Agent`]
pub struct AgentBuilder<DB: Database> {
  /// Identifier for this agent.
  /// Used for routing messages.
  pub id:    String,
  /// The engines/behaviors that the agent uses to sync, startup, and process
  /// events.
  behaviors: Vec<Box<dyn Behavior<DB>>>,
}

impl<DB: Database> AgentBuilder<DB>
where
  DB::Location: Send + Sync + 'static,
  DB::State: Send + Sync + 'static,
{
  /// Appends a behavior onto an [`AgentBuilder`]. Behaviors are initialized
  /// when the agent builder is added to the [`crate::world::World`]
  pub fn with_behavior(mut self, behavior: Box<dyn Behavior<DB>>) -> Self {
    self.behaviors.push(behavior);
    self
  }

  /// Constructs and returns a new [`Agent`] instance using the provided
  /// `client` and `messager`.
  ///
  /// This method finalizes the building process of an [`Agent`] by taking
  /// ownership of the builder, and attempting to construct an `Agent`
  /// with the accumulated configurations and the provided `client` and
  /// `messager`. The `client` is an [`Arc<RevmMiddleware>`] that represents
  /// the connection to the blockchain or environment, and `messager` is a
  /// communication layer for the agent.
  ///
  /// # Parameters
  ///
  /// - `client`: A shared [`Arc<RevmMiddleware>`] instance that provides the agent with access to
  ///   the blockchain or environment.
  /// - `messager`: A [`Messager`] instance for the agent to communicate with other agents or
  ///   systems.
  ///
  /// # Returns
  ///
  /// Returns a `Result` that, on success, contains the newly created
  /// [`Agent`] instance. On failure, it returns an
  /// [`AgentBuildError::MissingBehaviorEngines`] error indicating that the
  /// agent was attempted to be built without any behavior engines
  /// configured.
  ///
  /// # Examples
  ///
  /// ```ignore
  /// let agent_builder = AgentBuilder::new("agent_id");
  /// let client = Arc::new(RevmMiddleware::new(...));
  /// let messager = Messager::new(...);
  /// let agent = agent_builder.build(client, messager).expect("Failed to build agent");
  /// ```
  pub fn build(
    self,
    middleware: Middleware<DB>,
    messager: Messager,
  ) -> Result<Agent<DB>, ArbiterCoreError> {
    let stream = Stream {
      stream:  create_unified_stream(middleware.receiver, messager.broadcast_receiver),
      filters: HashMap::new(),
    };
    let sender = Sender {
      state_change_sender: middleware.sender,
      message_sender:      messager.broadcast_sender,
    };

    Ok(Agent { id: self.id, sender, stream, behaviors: self.behaviors })
  }
}

use futures::stream;

fn create_unified_stream<DB: Database>(
  middleware_receiver: broadcast::Receiver<(DB::Location, DB::State)>,
  messager_receiver: broadcast::Receiver<Message>,
) -> EventStream<Event<DB>>
where
  DB::Location: Send + Sync + 'static,
  DB::State: Send + Sync + 'static,
{
  let middleware_stream =
    Box::pin(stream::unfold(middleware_receiver, |mut receiver| async move {
      loop {
        match receiver.recv().await {
          Ok((location, state)) => return Some((Event::StateChange(location, state), receiver)),
          Err(broadcast::error::RecvError::Closed) => return None,
          Err(broadcast::error::RecvError::Lagged(_)) => {},
        }
      }
    }));

  let messager_stream = Box::pin(stream::unfold(messager_receiver, |mut receiver| async move {
    loop {
      match receiver.recv().await {
        Ok(message) =>
          return Some((
            Event::MessageFrom(MessageFrom { from: message.from, data: message.data }),
            receiver,
          )),
        Err(broadcast::error::RecvError::Closed) => return None,
        Err(broadcast::error::RecvError::Lagged(_)) => {},
      }
    }
  }));

  Box::pin(futures::stream::select(middleware_stream, messager_stream))
}
