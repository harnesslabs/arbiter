use super::*;
use crate::{
  environment::{Database, Middleware},
  machine::{Behavior, Engine, EngineType},
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

  pub messager: Messager,

  pub middleware: Middleware<DB>,

  pub(crate) engines: Vec<Box<dyn EngineType<DB>>>,
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
    AgentBuilder { id: id.to_owned(), behavior_engines: Vec::new() }
  }
}

/// [`AgentBuilder`] represents the intermediate state of agent creation before
/// it is converted into a full on [`Agent`]
pub struct AgentBuilder<DB: Database> {
  /// Identifier for this agent.
  /// Used for routing messages.
  pub id:           String,
  /// The engines/behaviors that the agent uses to sync, startup, and process
  /// events.
  behavior_engines: Vec<Box<dyn EngineType<DB>>>,
}

impl<DB: Database> AgentBuilder<DB> {
  /// Appends a behavior onto an [`AgentBuilder`]. Behaviors are initialized
  /// when the agent builder is added to the [`crate::world::World`]
  pub fn with_behavior<E, B>(mut self, behavior: B) -> Self
  where
    E: DeserializeOwned + Serialize + Send + Sync + Debug + 'static,
    B: Behavior<E> + Serialize + DeserializeOwned + Send + Sync + 'static,
    DB: Database + 'static,
    DB::Location: Send + Sync + 'static,
    DB::State: Send + Sync + 'static, {
    let engine = Engine::new(behavior);
    self.behavior_engines.push(Box::new(engine));
    self
  }

  /// Adds a state machine engine to the agent builder.
  ///
  /// This method allows for the addition of a custom state machine engine to
  /// the agent's behavior engines. If the agent builder already has some
  /// engines, the new engine is appended to the list. If no engines are
  /// present, a new list is created with the provided engine as its first
  /// element.
  ///
  /// # Parameters
  ///
  /// - `engine`: The state machine engine to be added to the agent builder. This engine must
  ///   implement the `StateMachine` trait and is expected to be provided as a boxed trait object to
  ///   allow for dynamic dispatch.
  ///
  /// # Returns
  ///
  /// Returns the `AgentBuilder` instance to allow for method chaining.
  pub(crate) fn with_engine(mut self, engine: Box<dyn EngineType<DB>>) -> Self {
    self.behavior_engines.push(engine);
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
    Ok(Agent { id: self.id, messager, middleware, engines: self.behavior_engines })
  }
}
