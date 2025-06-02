//! # Leader-Follower Agent Simulation with Arbiter-Core
//!
//! A WebAssembly library that demonstrates a multi-agent system using our custom
//! arbiter-core framework with dynamic agent lifecycle management.

#![cfg(target_arch = "wasm32")]

use std::{
  collections::HashMap,
  sync::{Arc, Mutex, OnceLock},
};

use arbiter_core::{
  agent::{Agent, LifeCycle},
  handler::Handler,
  runtime::Runtime,
};
use wasm_bindgen::prelude::*;
use web_sys::console;

// Enable better error messages in debug mode
extern crate console_error_panic_hook;

/// Simple PRNG for WASM compatibility
fn random() -> f64 {
  static mut SEED: u32 = 12345;
  unsafe {
    SEED = SEED.wrapping_mul(1_103_515_245).wrapping_add(12345);
    f64::from((SEED >> 16) & 0x7fff) / 32767.0
  }
}

/// Position in 2D space
#[derive(Clone, Debug)]
pub struct Position {
  pub x: f64,
  pub y: f64,
}

impl Position {
  pub const fn new(x: f64, y: f64) -> Self { Self { x, y } }

  pub fn distance_to(&self, other: &Self) -> f64 {
    let dx = self.x - other.x;
    let dy = self.y - other.y;
    dx.hypot(dy)
  }

  pub fn move_towards(&mut self, target: &Self, speed: f64) {
    let distance = self.distance_to(target);
    if distance > 0.0 {
      let dx = (target.x - self.x) / distance;
      let dy = (target.y - self.y) / distance;
      self.x += dx * speed;
      self.y += dy * speed;
    }
  }
}

/// Message to tick all agents (contains leader positions for followers)
#[derive(Clone, Copy)]
pub struct Tick;

// Global shared state accessible from both Rust and JavaScript
static SHARED_AGENT_STATE: OnceLock<Arc<Mutex<HashMap<String, (String, Position)>>>> =
  OnceLock::new();

fn get_shared_agent_state() -> &'static Arc<Mutex<HashMap<String, (String, Position)>>> {
  SHARED_AGENT_STATE.get_or_init(|| Arc::new(Mutex::new(HashMap::new())))
}

/// Initialize the WASM module
#[wasm_bindgen(start)]
pub fn main() {
  console_error_panic_hook::set_once();
  console::log_1(&"Leader-Follower Simulation WASM module initialized with Arbiter-Core!".into());
}

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

impl LifeCycle for Leader {}

impl Handler<Tick> for Leader {
  type Reply = ();

  fn handle(&mut self, _message: &Tick) -> Self::Reply {
    self.move_agent();

    // Write directly to shared state
    if let Ok(mut shared_agents) = get_shared_agent_state().lock() {
      shared_agents.insert(self.id.clone(), ("leader".to_string(), self.position.clone()));
    }
  }
}

/// Simple follower agent that follows the closest leader
#[derive(Clone)]
pub struct Follower {
  pub id:               String,
  pub position:         Position,
  pub speed:            f64,
  pub follow_distance:  f64,
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

impl LifeCycle for Follower {}

impl Handler<Tick> for Follower {
  type Reply = ();

  fn handle(&mut self, _message: &Tick) -> Self::Reply {
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

    // Write directly to shared state
    if let Ok(mut shared_agents) = get_shared_agent_state().lock() {
      shared_agents.insert(self.id.clone(), ("follower".to_string(), self.position.clone()));
    }
  }
}

/// Get all agent positions for rendering (called from JavaScript)
#[wasm_bindgen]
pub fn get_agent_positions() -> String {
  match get_shared_agent_state().lock() {
    Ok(shared_agents) => {
      let mut agents_json = String::from("[");
      let mut first = true;

      for (agent_id, (agent_type, position)) in shared_agents.iter() {
        if !first {
          agents_json.push(',');
        }
        first = false;

        agents_json.push_str(&format!(
          r#"{{"id":"{}","type":"{}","x":{},"y":{}}}"#,
          agent_id, agent_type, position.x, position.y
        ));
      }

      agents_json.push(']');
      agents_json
    },
    Err(_) => {
      console::log_1(&"❌ Failed to lock shared agent state".into());
      "[]".to_string()
    },
  }
}

// Global canvas height and width
static mut CANVAS_HEIGHT: f64 = 0.0;
static mut CANVAS_WIDTH: f64 = 0.0;

/// Initialize the leader-follower simulation with shared state
#[wasm_bindgen]
pub fn create_leader_follower_simulation(canvas_width: f64, canvas_height: f64) -> Runtime {
  console_error_panic_hook::set_once();

  let runtime = Runtime::new();

  unsafe {
    CANVAS_WIDTH = canvas_width;
    CANVAS_HEIGHT = canvas_height;
  }

  // Initialize shared state
  let _shared_state = get_shared_agent_state();
  console::log_1(&"🎨 Shared agent state initialized".into());

  runtime
}

/// Step the simulation forward by one tick
#[wasm_bindgen]
pub fn simulation_tick(runtime: &mut Runtime) {
  // Broadcast Tick to all agents
  runtime.broadcast_message(Tick);

  // Process tick messages and any resulting updates
  runtime.step();
}

/// Remove a single agent from shared state
#[wasm_bindgen]
pub fn remove_single_agent(agent_id: &str) {
  if let Ok(mut shared_agents) = get_shared_agent_state().lock() {
    if shared_agents.remove(agent_id).is_some() {
      console::log_1(&format!("🗑️ Removed {} from shared state", agent_id).into());
    }
  }
}

/// Clear all agents from shared state and reset counters
#[wasm_bindgen]
pub fn clear_all_agents() {
  // Clear shared state
  if let Ok(mut shared_agents) = get_shared_agent_state().lock() {
    shared_agents.clear();
    console::log_1(&"🧹 Cleared all agents from shared state".into());
  }

  // Reset counters
  unsafe {
    LEADER_COUNT = 0;
    FOLLOWER_COUNT = 0;
  }
}

// Global counters for agent IDs
static mut LEADER_COUNT: u32 = 0;
static mut FOLLOWER_COUNT: u32 = 0;

/// Add an agent at the specified position  
#[wasm_bindgen]
pub fn add_simulation_agent(runtime: &mut Runtime, x: f64, y: f64, is_leader: bool) -> String {
  let agent_id = if is_leader {
    unsafe {
      LEADER_COUNT += 1;
      format!("Leader {LEADER_COUNT}")
    }
  } else {
    unsafe {
      FOLLOWER_COUNT += 1;
      format!("Follower {FOLLOWER_COUNT}")
    }
  };

  let success = if is_leader {
    unsafe {
      let leader = Leader::new(agent_id.clone(), CANVAS_WIDTH, CANVAS_HEIGHT, x, y);
      let leader_agent = Agent::new(leader).with_handler::<Tick>();

      match runtime.spawn_named_agent(&agent_id, leader_agent) {
        Ok(_) => {
          console::log_1(&format!("🔴 {agent_id} created and started").into());
          true
        },
        Err(e) => {
          console::log_1(&format!("❌ Failed to register {agent_id}: {e}").into());
          false
        },
      }
    }
  } else {
    let follower = Follower::new(agent_id.clone(), x, y);
    let follower_agent = Agent::new(follower).with_handler::<Tick>();

    match runtime.spawn_named_agent(&agent_id, follower_agent) {
      Ok(_) => {
        console::log_1(&format!("🔵 {agent_id} created and started").into());
        true
      },
      Err(e) => {
        console::log_1(&format!("❌ Failed to register {agent_id}: {e}").into());
        false
      },
    }
  };

  if success {
    agent_id
  } else {
    String::new()
  }
}
