//! The world module contains the core world abstraction for the Arbiter Engine.

use std::collections::VecDeque;

use arbiter_core::{database::ArbiterDB, environment::Environment, middleware::ArbiterMiddleware};
use futures_util::future::join_all;
use serde::de::DeserializeOwned;
use tokio::spawn;

use super::*;
use crate::{
  agent::{Agent, AgentBuilder},
  machine::{CreateStateMachine, MachineInstruction},
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
#[derive(Debug)]
pub struct World {
  /// The identifier of the world.
  pub id: String,

  /// The agents in the world.
  pub agents: Option<HashMap<String, Agent>>,

  /// The environment for the world.
  pub environment: Option<Environment>,

  /// The messaging layer for the world.
  pub messager: Messager,
}

use std::{fs::File, io::Read};
impl World {
  /// Creates a new [`World`] with the given identifier and provider.
  pub fn new(id: &str) -> Self {
    Self {
      id:          id.to_owned(),
      agents:      Some(HashMap::new()),
      environment: Some(Environment::builder().build()),
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
  pub fn from_config<C: CreateStateMachine + Serialize + DeserializeOwned + Debug>(
    config_path: &str,
  ) -> Result<Self, ArbiterEngineError> {
    let cwd = std::env::current_dir()?;
    let path = cwd.join(config_path);
    info!("Reading from path: {:?}", path);
    let mut file = File::open(path)?;

    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    #[derive(Deserialize)]
    struct Config<C> {
      id:         Option<String>,
      #[serde(flatten)]
      agents_map: HashMap<String, Vec<C>>,
    }

    let config: Config<C> = toml::from_str(&contents)?;

    let mut world = World::new(&config.id.unwrap_or_else(|| "world".to_owned()));

    for (agent, behaviors) in config.agents_map {
      let mut next_agent = Agent::builder(&agent);
      for behavior in behaviors {
        let engine = behavior.create_state_machine();
        next_agent = next_agent.with_engine(engine);
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
  pub fn add_agent(&mut self, agent_builder: AgentBuilder) {
    let id = agent_builder.id.clone();
    let client = ArbiterMiddleware::new(self.environment.as_ref().unwrap(), Some(&id))
      .expect("Failed to create RevmMiddleware client for agent");
    let messager = self.messager.for_agent(&id);
    let agent =
      agent_builder.build(client, messager).expect("Failed to build agent from AgentBuilder");
    let agents = self.agents.as_mut().expect("Agents collection not initialized");
    agents.insert(id.to_owned(), agent);
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
  pub async fn run(&mut self) -> Result<ArbiterDB, ArbiterEngineError> {
    let agents = match self.agents.take() {
      Some(agents) => agents,
      None =>
        return Err(ArbiterEngineError::WorldError(
          "No agents found. Has the world already been ran?".to_owned(),
        )),
    };
    let mut tasks = vec![];
    // Prepare a queue for messagers corresponding to each behavior engine.
    let mut messagers = VecDeque::new();
    // Populate the messagers queue.
    for (_, agent) in agents.iter() {
      for _ in &agent.behavior_engines {
        messagers.push_back(agent.messager.clone());
      }
    }
    // For each agent, spawn a task for each of its behavior engines.
    // Unwrap here is safe as we just built the dang thing.
    for (_, mut agent) in agents {
      for mut engine in agent.behavior_engines.drain(..) {
        let client = agent.client.clone();
        let messager = messagers.pop_front().unwrap();
        tasks.push(spawn(async move {
          engine.execute(MachineInstruction::Start(client, messager)).await
        }));
      }
    }
    // Await the completion of all tasks.
    join_all(tasks).await;

    let db = self.environment.take().unwrap().stop()?;
    Ok(db)
  }
}
