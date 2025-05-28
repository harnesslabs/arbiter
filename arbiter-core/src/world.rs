use std::{collections::HashMap, fs::File, io::Read};

use tokio::sync::{broadcast, mpsc};
use tracing::info;

use super::*;
use crate::{
  agent::{Agent, AgentBuilder},
  environment::{Database, Environment, InMemoryEnvironment, Middleware},
  error::ArbiterCoreError,
  machine::ConfigurableBehavior,
  messager::{Message, Messager},
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
      environment: InMemoryEnvironment::new(32).unwrap(),
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
    let (state_tx, _) = mpsc::channel(32);
    let (broadcast_tx, broadcast_rx) = broadcast::channel::<Message>(32);

    let middleware = Middleware {
      sender:             state_tx,
      receiver:           self.environment.state_broadcast.subscribe(),
      broadcast_sender:   broadcast_tx,
      broadcast_receiver: broadcast_rx,
    };

    let agent = agent_builder.build(middleware).expect("Failed to build agent from AgentBuilder");
    self.agents.insert(agent.id.clone(), agent);
  }

  pub async fn run(mut self) -> Result<Self, ArbiterCoreError> {
    info!("Running world {}", self.id);

    let mut handles = Vec::new();
    let mut agents = std::mem::take(&mut self.agents);

    for (_, agent) in agents.drain() {
      let id = agent.id.clone();
      let sender = agent.sender.clone();
      let mut behaviors = agent.behaviors;
      let mut stream = agent.stream;

      handles.push(tokio::spawn(async move {
        info!("Starting agent {}", id);

        for behavior in behaviors.iter_mut() {
          let (filter, actions) = behavior.startup()?;

          if let Some(filter) = filter {
            stream.add_filter(id.clone(), filter);
          }

          sender.execute_actions(actions).await?;
        }

        while let Some(event) = stream.stream_mut().next().await {
          for behavior in behaviors.iter_mut() {
            let (control_flow, actions) = behavior.process_event(event.clone()).await?;
            sender.execute_actions(actions).await?;

            if matches!(control_flow, crate::machine::ControlFlow::Halt) {
              info!("Behavior halted for agent {}", id);
              break;
            }
          }
        }

        Ok::<(), ArbiterCoreError>(())
      }));
    }

    for handle in handles {
      handle.await??;
    }

    self.agents = agents;
    Ok(self)
  }
}
