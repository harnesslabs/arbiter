use std::collections::HashMap;

use arbiter_core::environment::Environment;

pub struct Canvas {
  width: f64,
  height: f64,
  pub agent_positions: HashMap<String, (f64, f64)>,
}

impl Environment for Canvas {
  // return an owned copy rather than a borrow tied to an unrelated `'a`
  type State = HashMap<String, (f64, f64)>;
  type Update = (String, (f64, f64)); // (agent_id, new_position)

  fn new() -> Self {
    Self { width: 800.0, height: 600.0, agent_positions: HashMap::new() }
  }

  fn get_state(&self) -> Self::State {
    self.agent_positions.clone()
  }

  fn update_state(&self, update: Self::Update) {
    let (agent_id, position) = update;
    self.agent_positions.insert(agent_id, position);
  }
}
