use super::*;
use crate::canvas::PositionUpdate;

/// Simple follower agent that follows the closest leader
#[derive(Clone)]
pub struct Follower {
  pub id: String,
  pub position: Position,
  pub speed: f64,
  pub follow_distance: f64,
  pub target_leader_id: Option<String>,
  pub leader_positions: HashMap<String, Position>, // Store leader positions
}

impl Follower {
  pub fn new(id: String, x: f64, y: f64) -> Self {
    Self {
      id,
      position: Position::new(x, y),
      speed: 0.8,
      follow_distance: 50.0,
      target_leader_id: None,
      leader_positions: HashMap::new(),
    }
  }

  fn find_closest_leader(&mut self) {
    if self.leader_positions.is_empty() {
      self.target_leader_id = None;
      console::log_1(&format!("🔵 {} has no leaders to follow", self.id).into());
      return;
    }

    let mut closest_distance = f64::INFINITY;
    let mut closest_leader = None;

    for (leader_id, leader_pos) in &self.leader_positions {
      let distance = self.position.distance_to(leader_pos);
      if distance < closest_distance {
        closest_distance = distance;
        closest_leader = Some(leader_id.clone());
      }
    }

    self.target_leader_id = closest_leader;
  }

  fn follow_target(&mut self) {
    if let Some(target_id) = &self.target_leader_id {
      if let Some(leader_pos) = self.leader_positions.get(target_id) {
        let distance = self.position.distance_to(leader_pos);
        if distance > self.follow_distance {
          self.position.move_towards(leader_pos, self.speed);
        }
      }
    }
  }
}

impl LifeCycle for Follower {
  type Snapshot = Position;
  type StartMessage = ();
  type StopMessage = ();

  fn on_start(&mut self) -> Self::StartMessage {
    console::log_1(
      &format!("🔵 {} started at ({:.2}, {:.2})", self.id, self.position.x, self.position.y).into(),
    );
  }

  fn on_stop(&mut self) -> Self::StopMessage {
    console::log_1(&format!("🛑 {} stopped", self.id).into());
  }

  fn snapshot(&self) -> Self::Snapshot {
    self.position.clone()
  }
}

impl Handler<Tick> for Follower {
  type Reply = PositionUpdate;

  fn handle(&mut self, _message: &Tick) -> Option<Self::Reply> {
    // Read leader positions directly from shared state instead of relying on messages
    if let Ok(shared_agents) = get_shared_agent_state().lock() {
      self.leader_positions.clear();
      for (agent_id, (agent_type, position)) in shared_agents.iter() {
        if agent_type == "leader" {
          self.leader_positions.insert(agent_id.clone(), position.clone());
        }
      }
    }

    // Use stored leader positions to follow
    self.find_closest_leader();
    self.follow_target();

    Some(PositionUpdate {
      id: self.id.clone(),
      agent_type: "follower".to_string(),
      position: self.position.clone(),
    })
  }
}
