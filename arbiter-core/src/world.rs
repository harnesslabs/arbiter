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
  pub environment: Option<InMemoryEnvironment<DB>>,

  /// The messaging layer for the world.
  pub messager: Messager,
}

impl<DB: Database> World<DB> {
  /// Creates a new [`World`] with the given identifier and provider.
  pub fn new(id: &str) -> Self {
    Self {
      id:          id.to_owned(),
      agents:      Some(HashMap::new()),
      environment: Some(InMemoryEnvironment::new(10).unwrap()),
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
    let middleware = self.environment.as_ref().unwrap().middleware();
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
    DB::Location: Send + Sync + Clone + 'static,
    DB::State: Send + Sync + Clone + 'static, {
    let agents = match self.agents.take() {
      Some(agents) => agents,
      None =>
        return Err(ArbiterCoreError::WorldError(
          "No agents found. Has the world already been ran?".to_owned(),
        )),
    };

    // Start the environment task
    let environment = self.environment.take().unwrap();
    let _environment_task = task::spawn(async move {
      if let Err(e) = environment.run().await {
        error!("Environment task failed: {:?}", e);
      }
    });

    let mut tasks = vec![];

    for (agent_id, agent) in agents {
      let Agent { id, sender, mut stream, behaviors } = agent;

      debug!("Starting agent: {}", agent_id);

      // Collect behaviors with their filters and startup actions
      let mut behavior_data = Vec::new();

      for (behavior_idx, mut behavior) in behaviors.into_iter().enumerate() {
        // Call startup and get optional filter and actions
        let (filter, startup_actions) = match behavior.startup() {
          Ok((filter, actions)) => (filter, actions),
          Err(e) => {
            error!("Startup failed for behavior {}: {:?}", behavior_idx, e);
            continue;
          },
        };

        // Execute startup actions if provided
        if !startup_actions.is_empty() {
          if let Err(e) = sender.execute_actions(startup_actions).await {
            error!("Failed to execute startup actions for behavior {}: {:?}", behavior_idx, e);
          }
        }

        // Store behavior data for event processing
        behavior_data.push((behavior_idx, behavior, filter));
      }

      // Create a single task per agent to handle event streaming and routing
      if behavior_data.is_empty() {
        debug!("No behaviors for agent: {}", agent_id);
        // Create a minimal task that completes immediately for agents with no behaviors
        let agent_task = task::spawn(async move {
          debug!("Agent {} has no behaviors, completing immediately", agent_id);
        });
        tasks.push(agent_task);
      } else {
        let agent_task = task::spawn(async move {
          use futures::StreamExt;

          // Get the event stream
          let event_stream = stream.stream_mut();

          while let Some(event) = event_stream.next().await {
            // Process each behavior and retain only those that don't halt
            behavior_data.retain_mut(|(behavior_id, behavior, filter)| {
              // If filter is None, process all events; if Some(filter), check the filter
              let should_process = match filter {
                None => true, // No filter means process all events
                Some(filter) => filter.filter(&event),
              };

              if should_process {
                debug!("Event matched filter for behavior: {}", behavior_id);

                // Process the event with this behavior
                match futures::executor::block_on(behavior.process_event(event.clone())) {
                  Ok((crate::machine::ControlFlow::Halt, actions)) => {
                    debug!("Behavior {} requested halt", behavior_id);

                    // Execute any final actions before halting
                    if !actions.is_empty() {
                      if let Err(e) = futures::executor::block_on(sender.execute_actions(actions)) {
                        error!(
                          "Failed to execute final actions for behavior {}: {:?}",
                          behavior_id, e
                        );
                      }
                    }

                    // Return false to remove this behavior
                    false
                  },
                  Ok((crate::machine::ControlFlow::Continue, actions)) => {
                    // Execute actions and continue processing
                    if !actions.is_empty() {
                      if let Err(e) = futures::executor::block_on(sender.execute_actions(actions)) {
                        error!("Failed to execute actions for behavior {}: {:?}", behavior_id, e);
                      }
                    }
                    // Return true to keep this behavior
                    true
                  },
                  Err(e) => {
                    error!("Error processing event for behavior {}: {:?}", behavior_id, e);
                    // Return true to keep this behavior despite the error
                    true
                  },
                }
              } else {
                // Event didn't match filter, keep the behavior
                true
              }
            });

            debug!("{} behaviors remaining after processing event", behavior_data.len());

            // If no behaviors remain, exit the event loop
            if behavior_data.is_empty() {
              debug!("No behaviors remaining for agent, exiting event loop");
              break;
            }
          }

          debug!("Event stream ended for agent: {}", agent_id);
        });

        tasks.push(agent_task);
      }
    }

    // Await the completion of all tasks
    join_all(tasks).await;

    Ok(())
  }
}
