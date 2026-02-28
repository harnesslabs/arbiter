use arbiter::{actor::LifeCycle, handler::Handler};
use web_sys::console;

use crate::{get_shared_agent_state, Position};

/// Message to update an agent's position in the shared canvas state
#[derive(Clone, Debug)]
pub struct PositionUpdate {
  pub id: String,
  pub agent_type: String,
  pub position: Position,
}

/// Message to remove an agent from the shared canvas state
#[derive(Clone, Debug)]
pub struct RemoveAgent {
  pub id: String,
}

/// The Canvas actor acts as the bridge between the Simulation and the JS frontend.
/// It receives position updates and stores them in the shared state.
pub struct Canvas;

impl Canvas {
  pub fn new() -> Self {
    Self
  }
}

impl LifeCycle for Canvas {
  type Snapshot = ();
  type StartMessage = ();
  type StopMessage = ();

  fn on_start(&mut self) -> Self::StartMessage {}

  fn on_stop(&mut self) -> Self::StopMessage {}

  fn snapshot(&self) -> Self::Snapshot {}
}

impl Handler<PositionUpdate> for Canvas {
  type Reply = ();

  fn handle(&mut self, message: &PositionUpdate) -> Option<Self::Reply> {
    if let Ok(mut shared_agents) = get_shared_agent_state().lock() {
      shared_agents
        .insert(message.id.clone(), (message.agent_type.clone(), message.position.clone()));
    }
    None
  }
}

impl Handler<RemoveAgent> for Canvas {
  type Reply = ();

  fn handle(&mut self, message: &RemoveAgent) -> Option<Self::Reply> {
    if let Ok(mut shared_agents) = get_shared_agent_state().lock() {
      shared_agents.remove(&message.id);
      console::log_1(&format!("🎨 Canvas removed {} from shared state", message.id).into());
    }
    None
  }
}
