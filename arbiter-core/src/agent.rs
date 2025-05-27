use super::*;
use crate::{
  environment::{Database, Middleware},
  machine::{Action, Behavior, EventStream, Filter},
  messager::Message,
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
  stream:  EventStream<Action<DB>>,
  filters: HashMap<String, Box<dyn Filter<DB>>>,
}

pub struct Sender<DB: Database> {
  state_change_sender: mpsc::Sender<(DB::Location, DB::State)>,
  message_sender:      broadcast::Sender<Message>,
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

impl<DB: Database> AgentBuilder<DB> {
  /// Appends a behavior onto an [`AgentBuilder`]. Behaviors are initialized
  /// when the agent builder is added to the [`crate::world::World`]
  pub fn with_behavior<B>(mut self, behavior: B) -> Self
  where
    B: Behavior<DB> + Send + Sync + 'static,
    DB: Database + 'static,
    DB::Location: Send + Sync + 'static,
    DB::State: Send + Sync + 'static, {
    self.behaviors.push(Box::new(behavior));
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
    let middleware_sender = middleware.sender;
    let middleware_receiver = middleware.receiver;
    let messager_sender = messager.broadcast_sender;
    let messager_receiver = messager.broadcast_receiver;
    let stream =
      Stream { stream: create_unified_stream(middleware, messager), filters: HashMap::new() };
    let sender = Sender {
      state_change_sender: middleware.sender.clone(),
      message_sender:      messager.broadcast_sender.clone(),
    };

    Ok(Agent { id: self.id, sender, stream, behaviors: self.behaviors })
  }
}

fn create_unified_stream<DB: Database>(
  middleware_receiver: broadcast::Receiver<(DB::Location, DB::State)>,
  messager_receiver: broadcast::Receiver<Message>,
) -> EventStream<Action<DB>> {
  let middleware_stream =
    middleware.stream().map(|(location, state)| Action::StateChange(location, state));
  let messager_stream = messager.stream().unwrap().map(|msg| Action::Message(msg));

  Box::pin(futures::stream::select(middleware_stream, messager_stream))
}
