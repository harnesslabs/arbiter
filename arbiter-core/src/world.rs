//! The world module contains the core world abstraction for the Arbiter Engine.

// TODO: I think the DB needs to have a transaction layer associated to it actually.

use std::{collections::VecDeque, fs::File, io::Read};

use super::*;
use crate::{
  agent::{Agent, AgentBuilder},
  environment::{Database, InMemoryEnvironment},
  machine::{Behavior, ConfigurableBehavior},
};

/// A world is a collection of agents that use the same type of provider, e.g.,
/// operate on the same blockchain or same `Environment`. The world is
/// responsible for managing the agents and their state transitions.
///
/// # How it works
/// The [`World`] holds on to a collection of [`Agent`]s and can run them all
/// concurrently when the [`run`] method is called. The [`World`] takes in
/// [`AgentBuilder`]s and when it does so, it creates [`Agent`]s that are now
/// connected to the world via a client ([`Arc<RevmMiddleware>`]) and a messager
/// ([`Messager`]).
pub struct World<DB: Database> {
  /// The identifier of the world.
  pub id: String,

  /// The agents in the world.
  pub agents: Option<HashMap<String, Agent<DB>>>,

  /// The environment for the world.
  pub environment: InMemoryEnvironment<DB>,

  /// The messaging layer for the world.
  pub messager: Messager,
}

impl<DB: Database> World<DB> {
  /// Creates a new [`World`] with the given identifier and provider.
  pub fn new(id: &str) -> Self {
    Self {
      id:          id.to_owned(),
      agents:      Some(HashMap::new()),
      environment: InMemoryEnvironment::new(10).unwrap(),
      messager:    Messager::new(),
    }
  }

  /// Builds and adds agents to the world from a configuration file.
  ///
  /// This method reads a configuration file specified by `config_path`, which
  /// should be a TOML file containing the definitions of agents and their
  /// behaviors. Each agent is identified by a unique string key, and
  /// associated with a list of behaviors. These behaviors are
  /// deserialized into instances that implement the `CreateStateMachine`
  /// trait, allowing them to be converted into state machines that define
  /// the agent's behavior within the world.
  ///
  /// # Type Parameters
  ///
  /// - `C`: The type of the behavior component that each agent will be associated with. This type
  ///   must implement the `CreateStateMachine`, `Serialize`, `DeserializeOwned`, and `Debug`
  ///   traits.
  ///
  /// # Arguments
  ///
  /// - `config_path`: A string slice that holds the path to the configuration file relative to the
  ///   current working directory.
  ///
  /// # Panics
  ///
  /// This method will panic if:
  /// - The current working directory cannot be determined.
  /// - The configuration file specified by `config_path` cannot be opened.
  /// - The configuration file cannot be read into a string.
  /// - The contents of the configuration file cannot be deserialized into the expected
  ///   `HashMap<String, Vec<C>>` format.
  ///
  /// # Examples
  ///
  /// Assuming a TOML file named `agents_config.toml` exists in the current
  /// working directory with the following content:
  ///
  /// ```toml
  /// [[agent1]]
  /// BehaviorTypeA = { ... } ,
  /// [[agent1]]
  /// BehaviorTypeB = { ... }
  ///
  /// [agent2]
  /// BehaviorTypeC = { ... }
  /// ```
  pub fn from_config<C>(config_path: &str) -> Result<Self, ArbiterCoreError>
  where
    C: ConfigurableBehavior<DB> + Serialize + DeserializeOwned + Debug,
    DB: Database + 'static,
    DB::Location: Send + Sync + 'static,
    DB::State: Send + Sync + 'static, {
    let cwd = std::env::current_dir().unwrap();
    let path = cwd.join(config_path);
    info!("Reading from path: {:?}", path);
    let mut file = File::open(path).unwrap();

    let mut contents = String::new();
    file.read_to_string(&mut contents).unwrap();

    #[derive(Deserialize)]
    struct Config<C> {
      id:         Option<String>,
      #[serde(flatten)]
      agents_map: HashMap<String, Vec<C>>,
    }

    let config: Config<C> = toml::from_str(&contents).unwrap();

    let mut world = World::new(&config.id.unwrap_or_else(|| "world".to_owned()));

    for (agent, behaviors) in config.agents_map {
      let mut next_agent = Agent::builder(&agent);
      for behavior in behaviors {
        next_agent = next_agent.with_behavior(behavior.create_behavior());
      }
      world.add_agent(next_agent);
    }
    Ok(world)
  }

  /// Adds an agent, constructed from the provided `AgentBuilder`, to the
  /// world.
  ///
  /// This method takes an `AgentBuilder` instance, extracts its identifier,
  /// and uses it to create both a `RevmMiddleware` client and a
  /// `Messager` specific to the agent. It then builds the `Agent` from
  /// the `AgentBuilder` using these components. Finally, the newly
  /// created `Agent` is inserted into the world's internal collection of
  /// agents.
  ///
  /// # Panics
  ///
  /// This method will panic if:
  /// - It fails to create a `RevmMiddleware` client for the agent.
  /// - The `AgentBuilder` fails to build the `Agent`.
  /// - The world's internal collection of agents is not initialized.
  ///
  /// # Examples
  ///
  /// Assuming you have an `AgentBuilder` instance named `agent_builder`:
  ///
  /// ```ignore
  /// world.add_agent(agent_builder);
  /// ```
  ///
  /// This will add the agent defined by `agent_builder` to the world.
  pub fn add_agent(&mut self, agent_builder: AgentBuilder<DB>)
  where
    DB: Database + 'static,
    DB::Location: Send + Sync + 'static,
    DB::State: Send + Sync + 'static, {
    let id = agent_builder.id.clone();
    let middleware = self.environment.middleware();
    let messager = self.messager.for_agent(&id);
    let agent =
      agent_builder.build(middleware, messager).expect("Failed to build agent from AgentBuilder");
    self.agents.as_mut().unwrap().insert(id.to_owned(), agent);
  }

  /// Executes all agents and their behaviors concurrently within the world.
  ///
  /// This method takes all the agents registered in the world and runs their
  /// associated behaviors in parallel. Each agent's behaviors are
  /// executed with their respective messaging and client context. This
  /// method ensures that all agents and their behaviors are started
  /// simultaneously, leveraging asynchronous execution to manage concurrent
  /// operations.
  ///
  /// # Errors
  ///
  /// Returns an error if no agents are found in the world, possibly
  /// indicating that the world has already been run or that no agents
  /// were added prior to execution.
  pub async fn run(&mut self) -> Result<(), ArbiterCoreError>
  where
    DB: Database + 'static,
    DB::Location: Send + Sync + 'static,
    DB::State: Send + Sync + 'static, {
    let agents = match self.agents.take() {
      Some(agents) => agents,
      None =>
        return Err(ArbiterCoreError::WorldError(
          "No agents found. Has the world already been ran?".to_owned(),
        )),
    };
    let mut tasks = vec![];

    for (_, agent) in agents {
      for mut behavior in agent.behaviors.drain(..) {
        let (filter, actions) = behavior.startup().unwrap();
        // TODO: Add all the filters to the stream
        // TODO: Start the agent streaming and passing actions to the behavior matching filters then
        // processing
        // TODO: In the same task (one per agent), send all the actions.
        agent.stream.add_filter(filter);
      }
    }
    // Await the completion of all tasks.
    join_all(tasks).await;

    Ok(())
  }
}
