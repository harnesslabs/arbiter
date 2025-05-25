use super::*;
use crate::machine::{Behavior, Engine, StateMachine};

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
#[derive(Debug)]
pub struct Agent {
  pub id: String,

  pub messager: Messager,

  pub middleware: Middleware,

  pub(crate) behavior_engines: Vec<Box<dyn StateMachine>>,
}

impl Agent {
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
  pub fn builder(id: &str) -> AgentBuilder {
    AgentBuilder { id: id.to_owned(), behavior_engines: None }
  }
}

/// [`AgentBuilder`] represents the intermediate state of agent creation before
/// it is converted into a full on [`Agent`]
pub struct AgentBuilder {
  /// Identifier for this agent.
  /// Used for routing messages.
  pub id:           String,
  /// The engines/behaviors that the agent uses to sync, startup, and process
  /// events.
  behavior_engines: Option<Vec<Box<dyn StateMachine>>>,
}

impl AgentBuilder {
  /// Appends a behavior onto an [`AgentBuilder`]. Behaviors are initialized
  /// when the agent builder is added to the [`crate::world::World`]
  pub fn with_behavior<E: DeserializeOwned + Serialize + Send + Sync + Debug + 'static>(
    mut self,
    behavior: impl Behavior<E> + 'static,
  ) -> Self {
    let engine = Engine::new(behavior);
    if let Some(engines) = &mut self.behavior_engines {
      engines.push(Box::new(engine));
    } else {
      self.behavior_engines = Some(vec![Box::new(engine)]);
    }
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
  pub(crate) fn with_engine(mut self, engine: Box<dyn StateMachine>) -> Self {
    if let Some(engines) = &mut self.behavior_engines {
      engines.push(engine);
    } else {
      self.behavior_engines = Some(vec![engine]);
    }
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
    middleware: Middleware,
    messager: Messager,
  ) -> Result<Agent, ArbiterEngineError> {
    match self.behavior_engines {
      Some(engines) => Ok(Agent { id: self.id, messager, middleware, behavior_engines: engines }),
      None => Err(ArbiterEngineError::AgentBuildError("Missing behavior engines".to_owned())),
    }
  }
}
