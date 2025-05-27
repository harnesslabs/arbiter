use std::{fs::File, io::Read};

use super::*;
use crate::{
  agent::{Agent, AgentBuilder},
  environment::{Database, InMemoryEnvironment},
  machine::ConfigurableBehavior,
  messager::Messager,
};

pub struct World<DB: Database> {
  pub id:          String,
  pub agents:      HashMap<String, Agent<DB>>,
  pub environment: InMemoryEnvironment<DB>,
  pub messager:    Messager,
}

impl<DB> World<DB>
where
  DB: Database + 'static,
  DB::Location: Send + Sync + 'static,
  DB::State: Send + Sync + 'static,
{
  pub fn new(id: &str) -> Self {
    Self {
      id:          id.to_owned(),
      agents:      HashMap::new(),
      environment: InMemoryEnvironment::new(1000).unwrap(),
      messager:    Messager::new(),
    }
  }

  pub fn from_config<C>(config_path: &str) -> Result<Self, ArbiterCoreError>
  where C: ConfigurableBehavior<DB> + 'static {
    #[derive(Deserialize)]
    struct Config<C> {
      id:         Option<String>,
      #[serde(flatten)]
      agents_map: HashMap<String, Vec<C>>,
    }

    let cwd = std::env::current_dir().unwrap();
    let path = cwd.join(config_path);
    info!("Reading from path: {:?}", path);
    let mut file = File::open(path).unwrap();

    let mut contents = String::new();
    file.read_to_string(&mut contents).unwrap();

    let config: Config<C> = toml::from_str(&contents).unwrap();

    let mut world = Self::new(&config.id.unwrap_or_else(|| "world".to_owned()));

    for (agent, behaviors) in config.agents_map {
      let mut next_agent = Agent::builder(&agent);
      for behavior in behaviors {
        next_agent = next_agent.with_behavior_from_config(behavior);
      }
      world.add_agent(next_agent);
    }
    Ok(world)
  }

  pub fn add_agent(&mut self, agent_builder: AgentBuilder<DB>) {
    let id = agent_builder.id.clone();
    let middleware = self.environment.middleware();
    let messager = self.messager.for_agent(&id);
    let agent =
      agent_builder.build(middleware, messager).expect("Failed to build agent from AgentBuilder");
    self.agents.insert(id.to_owned(), agent);
  }

  // TODO: Make this return a world, or env, or db or something that contains the sim data.
  pub async fn run(mut self) -> Result<Self, ArbiterCoreError> {
    // Get the agents
    let agents = std::mem::take(&mut self.agents);
    if agents.is_empty() {
      return Err(ArbiterCoreError::WorldError(
        "No agents found. Has the world already been ran?".to_owned(),
      ));
    }

    // Start the environment and get the shutdown sender
    let environment_task = self.environment.run().unwrap();

    let mut tasks = vec![];

    for (agent_id, agent) in agents {
      let Agent { id, sender, mut stream, behaviors } = agent;

      debug!("Starting agent: {}", id);

      // Collect behaviors with filters and startup actions
      let mut behavior_data = Vec::new();

      // TODO: We should add more debugging here eventually.
      for mut behavior in behaviors {
        match behavior.startup() {
          Ok((filter, actions)) =>
            if let Some(filter) = filter {
              behavior_data.push((behavior, filter));
              futures::executor::block_on(sender.execute_actions(actions));
            },
          Err(e) => panic!("a behavior failed to startup: {e:?}"),
        }
      }

      // Only create a task if there are behaviors that want to process events
      if behavior_data.is_empty() {
        debug!("Agent {} has no processing behaviors, not creating task", agent_id);
      } else {
        let agent_task = task::spawn(async move {
          debug!("Agent {} has {} processing behaviors", agent_id, behavior_data.len());

          // Get the event stream
          let event_stream = stream.stream_mut();

          while let Some(event) = event_stream.next().await {
            // Process each behavior and retain only those that don't halt
            behavior_data.retain_mut(|(behavior, filter)| {
              // Check if this behavior's filter matches the event
              if filter.filter(&event) {
                debug!("Event matched filter for behavior");

                // Process the event with this behavior
                match futures::executor::block_on(behavior.process_event(event.clone())) {
                  Ok((crate::machine::ControlFlow::Halt, actions)) => {
                    debug!("Behavior requested halt");

                    // Execute any final actions before halting
                    if !actions.is_empty() {
                      if let Err(e) = futures::executor::block_on(sender.execute_actions(actions)) {
                        error!("Failed to execute final actions for behavior: {:?}", e);
                      }
                    }

                    // Return false to remove this behavior
                    false
                  },
                  Ok((crate::machine::ControlFlow::Continue, actions)) => {
                    // Execute actions and continue processing
                    if !actions.is_empty() {
                      if let Err(e) = futures::executor::block_on(sender.execute_actions(actions)) {
                        error!("Failed to execute actions for behavior: {:?}", e);
                      }
                    }
                    // Return true to keep this behavior
                    true
                  },
                  Err(e) => {
                    error!("Error processing event for behavior: {:?}", e);
                    // Return true to keep this behavior despite the error
                    true
                  },
                }
              } else {
                // Event didn't match filter, keep the behavior
                true
              }
            });

            debug!("{} behaviors remaining for agent {}", behavior_data.len(), agent_id);

            // If no behaviors remain, exit the event loop
            if behavior_data.is_empty() {
              debug!("No behaviors remaining for agent {}, exiting event loop", agent_id);
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

    // Wait for the environment task to complete and get the final database
    debug!("All agent tasks completed, waiting for environment task to complete");
    match environment_task.await {
      Ok(final_database) => {
        // Reconstruct the environment with the final database state
        self.environment = InMemoryEnvironment::with_database(final_database, 1000);
      },
      Err(e) => {
        panic!("Environment task failed: {e:?}");
      },
    }

    // Return the world with its final state
    Ok(self)
  }
}
