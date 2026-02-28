use super::*;
use crate::canvas::PositionUpdate;

/// Simple leader agent that moves randomly
#[derive(Clone)]
pub struct Leader {
  pub id:                  String,
  pub position:            Position,
  pub canvas_width:        f64,
  pub canvas_height:       f64,
  pub speed:               f64,
  pub current_direction:   f64,
  pub direction_steps:     u32,
  pub max_direction_steps: u32,
}

impl Leader {
  pub fn new(id: String, canvas_width: f64, canvas_height: f64, x: f64, y: f64) -> Self {
    Self {
      id,
      position: Position::new(x, y),
      canvas_width,
      canvas_height,
      speed: 1.3,
      current_direction: random() * 2.0 * std::f64::consts::PI,
      direction_steps: 0,
      max_direction_steps: (100 + (random() * 100.0) as u32),
    }
  }

  fn update_direction(&mut self) {
    if self.direction_steps >= self.max_direction_steps {
      let direction_change = (random() - 0.5) * 0.5;
      self.current_direction += direction_change;
      self.current_direction %= 2.0 * std::f64::consts::PI;
      self.direction_steps = 0;
      self.max_direction_steps = 100 + (random() * 100.0) as u32;
    }
  }

  fn move_agent(&mut self) {
    self.update_direction();

    let dx = self.current_direction.cos() * self.speed;
    let dy = self.current_direction.sin() * self.speed;

    let mut new_pos = Position::new(self.position.x + dx, self.position.y + dy);

    // Bounce off walls
    let margin = 10.0;
    if new_pos.x < margin || new_pos.x > self.canvas_width - margin {
      self.current_direction = std::f64::consts::PI - self.current_direction;
      new_pos.x = new_pos.x.max(margin).min(self.canvas_width - margin);
      self.direction_steps = 0;
    }

    if new_pos.y < margin || new_pos.y > self.canvas_height - margin {
      self.current_direction = -self.current_direction;
      new_pos.y = new_pos.y.max(margin).min(self.canvas_height - margin);
      self.direction_steps = 0;
    }

    self.position = new_pos;
    self.direction_steps += 1;
  }
}

impl LifeCycle for Leader {
  type Snapshot = Position;
  type StartMessage = ();
  type StopMessage = ();

  fn on_start(&mut self) -> Self::StartMessage {
    console::log_1(
      &format!("🔴 {} started at ({:.2}, {:.2})", self.id, self.position.x, self.position.y).into(),
    );
  }

  fn on_stop(&mut self) -> Self::StopMessage {
    console::log_1(&format!("🛑 {} stopped", self.id).into());
  }

  fn snapshot(&self) -> Self::Snapshot { self.position.clone() }
}

impl Handler<Tick> for Leader {
  type Reply = PositionUpdate;

  fn handle(&mut self, _message: &Tick) -> Option<Self::Reply> {
    self.move_agent();

    Some(PositionUpdate {
      id:         self.id.clone(),
      agent_type: "leader".to_string(),
      position:   self.position.clone(),
    })
  }
}
