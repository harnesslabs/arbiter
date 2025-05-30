//! # Leader-Follower Agent Simulation with Arbiter-Core
//!
//! A WebAssembly library that demonstrates a multi-agent system using our custom
//! arbiter-core framework with dynamic agent lifecycle management.

#![cfg(target_arch = "wasm32")]

use std::collections::HashMap;

use arbiter_core::{
  agent::{Agent, LifeCycle},
  handler::Handler,
  runtime::Runtime,
};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use web_sys::{console, CanvasRenderingContext2d, HtmlCanvasElement};

// Enable better error messages in debug mode
extern crate console_error_panic_hook;

/// Generate a pseudo-random number between 0 and 1
fn random() -> f64 {
  static mut SEED: u32 = 12345;
  unsafe {
    SEED = SEED.wrapping_mul(1103515245).wrapping_add(12345);
    ((SEED >> 16) & 0x7fff) as f64 / 32767.0
  }
}

/// Position in 2D space
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
  pub x: f64,
  pub y: f64,
}

impl Position {
  pub fn new(x: f64, y: f64) -> Self { Self { x, y } }

  pub fn distance_to(&self, other: &Position) -> f64 {
    let dx = self.x - other.x;
    let dy = self.y - other.y;
    (dx * dx + dy * dy).sqrt()
  }

  pub fn move_towards(&mut self, target: &Position, speed: f64) {
    let distance = self.distance_to(target);
    if distance > 0.0 {
      let dx = (target.x - self.x) / distance;
      let dy = (target.y - self.y) / distance;
      self.x += dx * speed;
      self.y += dy * speed;
    }
  }
}

/// Agent types in our simulation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AgentType {
  Leader,
  Follower,
}

// === MESSAGE TYPES FOR OUR SYSTEM ===

/// Message to update agent position
#[derive(Clone, Debug)]
pub struct UpdatePosition {
  pub agent_id: String,
  pub position: Position,
}

/// Message to tick the simulation (move agents) - enriched with leader positions
#[derive(Clone, Debug)]
pub struct Tick {
  pub leader_positions: Vec<(String, Position)>,
}

/// Message to render all agents
#[derive(Clone, Debug)]
pub struct RenderAll {
  pub agent_positions: Vec<(String, Position, AgentType)>,
}

/// Message to register an agent type with canvas
#[derive(Clone, Debug)]
pub struct RegisterAgentType {
  pub agent_id:   String,
  pub agent_type: AgentType,
}

/// Message to remove an agent from canvas tracking
#[derive(Clone, Debug)]
pub struct RemoveAgent {
  pub agent_id: String,
}

/// Message to clear all agents from canvas tracking
#[derive(Clone, Debug)]
pub struct ClearAllAgents;

/// Message containing leader positions for followers
#[derive(Clone, Debug)]
pub struct LeaderPositions {
  pub positions: Vec<(String, Position)>,
}

// === AGENT IMPLEMENTATIONS ===

/// Canvas agent that manages world state and rendering
#[derive(Clone)]
pub struct Canvas {
  pub canvas_width:    f64,
  pub canvas_height:   f64,
  pub agent_positions: HashMap<String, Position>,
  pub agent_types:     HashMap<String, AgentType>,
}

impl Canvas {
  pub fn new(canvas_width: f64, canvas_height: f64) -> Self {
    Self {
      canvas_width,
      canvas_height,
      agent_positions: HashMap::new(),
      agent_types: HashMap::new(),
    }
  }

  fn get_canvas_context(&self) -> Option<CanvasRenderingContext2d> {
    let window = web_sys::window()?;
    let document = window.document()?;
    let canvas = document.get_element_by_id("canvas")?;
    let canvas: HtmlCanvasElement = canvas.dyn_into().ok()?;
    let context = canvas.get_context("2d").ok()??;
    let context: CanvasRenderingContext2d = context.dyn_into().ok()?;
    Some(context)
  }

  fn render(&self) {
    if let Some(context) = self.get_canvas_context() {
      // Clear any existing drawings first
      context.clear_rect(0.0, 0.0, self.canvas_width, self.canvas_height);

      // Fill with white background
      context.set_fill_style_str("#ffffff");
      context.fill_rect(0.0, 0.0, self.canvas_width, self.canvas_height);

      // Draw agents (if any)
      for (agent_id, position) in &self.agent_positions {
        let agent_type = self.agent_types.get(agent_id);

        match agent_type {
          Some(AgentType::Leader) => {
            // Draw leader as red circle
            context.set_fill_style_str("#ff4444");
            context.begin_path();
            context.arc(position.x, position.y, 12.0, 0.0, 2.0 * std::f64::consts::PI).unwrap();
            context.fill();
          },
          Some(AgentType::Follower) => {
            // Draw follower as blue circle
            context.set_fill_style_str("#4444ff");
            context.begin_path();
            context.arc(position.x, position.y, 8.0, 0.0, 2.0 * std::f64::consts::PI).unwrap();
            context.fill();
          },
          None => {
            // Unknown agent type, draw as gray
            context.set_fill_style_str("#888888");
            context.begin_path();
            context.arc(position.x, position.y, 6.0, 0.0, 2.0 * std::f64::consts::PI).unwrap();
            context.fill();
          },
        }
      }
    }
  }

  fn get_leader_positions(&self) -> Vec<(String, Position)> {
    let leaders: Vec<(String, Position)> = self
      .agent_positions
      .iter()
      .filter_map(|(id, pos)| {
        if let Some(agent_type) = self.agent_types.get(id) {
          if matches!(agent_type, AgentType::Leader) {
            Some((id.clone(), pos.clone()))
          } else {
            None
          }
        } else {
          None
        }
      })
      .collect();

    leaders
  }
}

impl LifeCycle for Canvas {}

impl Handler<UpdatePosition> for Canvas {
  type Reply = ();

  fn handle(&mut self, message: UpdatePosition) -> Self::Reply {
    self.agent_positions.insert(message.agent_id, message.position);
    self.render();
  }
}

impl Handler<RegisterAgentType> for Canvas {
  type Reply = ();

  fn handle(&mut self, message: RegisterAgentType) -> Self::Reply {
    self.agent_types.insert(message.agent_id.clone(), message.agent_type.clone());
    self.render();
  }
}

impl Handler<RemoveAgent> for Canvas {
  type Reply = ();

  fn handle(&mut self, message: RemoveAgent) -> Self::Reply {
    self.agent_positions.remove(&message.agent_id);
    self.agent_types.remove(&message.agent_id);
    self.render();
  }
}

impl Handler<ClearAllAgents> for Canvas {
  type Reply = ();

  fn handle(&mut self, _message: ClearAllAgents) -> Self::Reply {
    console::log_1(&"🎨 Canvas clearing all agents".into());

    // Clear all agent tracking data
    self.agent_positions.clear();
    self.agent_types.clear();

    console::log_1(
      &format!(
        "🎨 Canvas cleared - {} positions, {} types",
        self.agent_positions.len(),
        self.agent_types.len()
      )
      .into(),
    );

    // Re-render to show empty canvas
    self.render();
  }
}

impl Handler<Tick> for Canvas {
  type Reply = Tick;

  fn handle(&mut self, _message: Tick) -> Self::Reply {
    let leader_positions = self.get_leader_positions();
    Tick { leader_positions }
  }
}

/// Main simulation structure using arbiter-core
#[wasm_bindgen]
pub struct LeaderFollowerSimulation {
  canvas_width:   f64,
  canvas_height:  f64,
  leader_count:   u32,
  follower_count: u32,
  runtime:        Runtime,
}

#[wasm_bindgen]
impl LeaderFollowerSimulation {
  /// Create a new simulation instance
  #[wasm_bindgen(constructor)]
  pub fn new(canvas_width: f64, canvas_height: f64) -> Self {
    console_error_panic_hook::set_once();

    let mut runtime = Runtime::new();

    // Create and register Canvas agent with all its handlers
    let canvas = Canvas::new(canvas_width, canvas_height);
    let canvas_agent = Agent::new(canvas)
      .with_handler::<UpdatePosition>()
      .with_handler::<RegisterAgentType>()
      .with_handler::<RemoveAgent>()
      .with_handler::<ClearAllAgents>()
      .with_handler::<Tick>();

    // Register and start the canvas agent
    runtime.register_named_agent("canvas", canvas_agent).unwrap();
    runtime.start_agent_by_name("canvas").unwrap();

    console::log_1(&"🎨 Canvas initialized and started".into());

    // Send initial tick to set up the system
    runtime.broadcast_message(Tick { leader_positions: Vec::new() });
    runtime.step();

    // Start the automatic tick loop to keep agents moving
    Self::start_tick_loop(&mut runtime);

    Self { canvas_width, canvas_height, leader_count: 0, follower_count: 0, runtime }
  }

  /// Start the automatic tick loop to drive the simulation
  fn start_tick_loop(_runtime: &mut Runtime) {
    // Note: We can't move the runtime into the closure since we need it in the main struct
    // Instead, we'll rely on the manual tick() calls from JavaScript
    // Or we could use a different pattern, but for simplicity, let's use manual ticking
    console::log_1(&"🔄 Tick loop will be driven by JavaScript timer".into());
  }

  /// Step the simulation forward by one tick
  #[wasm_bindgen(js_name = "tick")]
  pub fn tick(&mut self) -> usize {
    // Use step() instead of run() to process just one round of messages
    // This allows for proper tick-by-tick simulation
    let delivered = self.runtime.broadcast_message(Tick { leader_positions: Vec::new() });

    if delivered > 0 {
      // Process one step of the simulation
      let processed = self.runtime.step();
      console::log_1(
        &format!("🔄 Tick: delivered {} messages, processed {}", delivered, processed).into(),
      );
      processed
    } else {
      console::log_1(&"⚠️ No agents to receive tick messages".into());
      0
    }
  }

  /// Add an agent at the specified position (simulation-specific)
  #[wasm_bindgen]
  pub fn add_agent(&mut self, x: f64, y: f64, is_leader: bool) -> String {
    let (agent_id, agent_type) = if is_leader {
      self.leader_count += 1;
      (format!("Leader {}", self.leader_count), AgentType::Leader)
    } else {
      self.follower_count += 1;
      (format!("Follower {}", self.follower_count), AgentType::Follower)
    };

    let success = match &agent_type {
      AgentType::Leader => {
        let leader = Leader::new(agent_id.clone(), self.canvas_width, self.canvas_height, x, y);
        let leader_agent = Agent::new(leader).with_handler::<Tick>();

        // Register and start the leader
        match self.runtime.spawn_named_agent(&agent_id, leader_agent) {
          Ok(_) => {
            console::log_1(&format!("🔴 {} created and started", agent_id).into());
            true
          },
          Err(e) => {
            console::log_1(&format!("❌ Failed to register {}: {}", agent_id, e).into());
            false
          },
        }
      },
      AgentType::Follower => {
        let follower = Follower::new(agent_id.clone(), x, y);
        let follower_agent = Agent::new(follower).with_handler::<Tick>();

        // Register and start the follower
        match self.runtime.spawn_named_agent(&agent_id, follower_agent) {
          Ok(_) => {
            console::log_1(&format!("🔵 {} created and started", agent_id).into());
            true
          },
          Err(e) => {
            console::log_1(&format!("❌ Failed to register {}: {}", agent_id, e).into());
            false
          },
        }
      },
    };

    if success {
      // Send agent type registration to Canvas
      self.runtime.broadcast_message(RegisterAgentType {
        agent_id:   agent_id.clone(),
        agent_type: agent_type.clone(),
      });

      // Send initial position update to Canvas
      self.runtime.broadcast_message(UpdatePosition {
        agent_id: agent_id.clone(),
        position: Position::new(x, y),
      });

      // Process all messages
      self.runtime.step();

      agent_id
    } else {
      String::new()
    }
  }

  /// Get agent count
  #[wasm_bindgen(js_name = "getAgentCount")]
  pub fn get_agent_count(&self) -> usize { self.runtime.agent_count() }

  /// Get agent names as JSON
  #[wasm_bindgen(js_name = "getAgentNames")]
  pub fn get_agent_names(&self) -> String {
    let names: Vec<&String> = self.runtime.agent_names();
    serde_json::to_string(&names).unwrap_or_else(|_| "[]".to_string())
  }

  /// Start an agent by name
  #[wasm_bindgen(js_name = "startAgent")]
  pub fn start_agent(&mut self, agent_name: &str) -> bool {
    self.runtime.start_agent_by_name(agent_name).is_ok()
  }

  /// Pause an agent by name
  #[wasm_bindgen(js_name = "pauseAgent")]
  pub fn pause_agent(&mut self, agent_name: &str) -> bool {
    self.runtime.pause_agent_by_name(agent_name).is_ok()
  }

  /// Resume an agent by name
  #[wasm_bindgen(js_name = "resumeAgent")]
  pub fn resume_agent(&mut self, agent_name: &str) -> bool {
    self.runtime.resume_agent_by_name(agent_name).is_ok()
  }

  /// Stop an agent by name
  #[wasm_bindgen(js_name = "stopAgent")]
  pub fn stop_agent(&mut self, agent_name: &str) -> bool {
    self.runtime.stop_agent_by_name(agent_name).is_ok()
  }

  /// Get agent state by name
  #[wasm_bindgen(js_name = "getAgentState")]
  pub fn get_agent_state(&self, agent_name: &str) -> String {
    match self.runtime.agent_state_by_name(agent_name) {
      Some(state) => format!("{:?}", state),
      None => "NotFound".to_string(),
    }
  }

  /// Remove an agent by ID
  #[wasm_bindgen]
  pub fn remove_agent(&mut self, agent_id: &str) -> bool {
    // Send message to canvas to remove the agent from display
    let remove_msg = RemoveAgent { agent_id: agent_id.to_string() };
    self.runtime.send_to_agent_by_name("canvas", remove_msg).is_ok();

    // Remove from runtime
    if self.runtime.remove_agent_by_name(agent_id).is_ok() {
      // Update counts
      if agent_id.starts_with("Leader") {
        self.leader_count = self.leader_count.saturating_sub(1);
      } else if agent_id.starts_with("Follower") {
        self.follower_count = self.follower_count.saturating_sub(1);
      }
      true
    } else {
      false
    }
  }

  /// Clear all agents (except canvas)
  #[wasm_bindgen]
  pub fn clear_all_agents(&mut self) -> usize {
    // Send message to canvas to clear all agents
    self.runtime.send_to_agent_by_name("canvas", ClearAllAgents).ok();

    // Get all agent names except canvas
    let all_names = self.runtime.agent_names();
    let agent_names_to_remove: Vec<String> = all_names
      .iter()
      .filter(|name| name.as_str() != "canvas")
      .map(|name| (*name).clone())
      .collect();

    // Remove each agent from runtime
    for agent_name in &agent_names_to_remove {
      let _ = self.runtime.remove_agent_by_name(agent_name);
    }

    // Reset agent counters
    self.leader_count = 0;
    self.follower_count = 0;

    agent_names_to_remove.len()
  }
}

/// Initialize the WASM module
#[wasm_bindgen(start)]
pub fn main() {
  console_error_panic_hook::set_once();
  console::log_1(
    &"🦀 Leader-Follower Simulation WASM module initialized with Arbiter-Core!".into(),
  );
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
      speed: 0.01,
      current_direction: random() * 2.0 * std::f64::consts::PI,
      direction_steps: 0,
      max_direction_steps: (100 + (random() * 100.0) as u32),
    }
  }

  fn update_direction(&mut self) {
    if self.direction_steps >= self.max_direction_steps {
      let direction_change = (random() - 0.5) * 0.5;
      self.current_direction += direction_change;
      self.current_direction = self.current_direction % (2.0 * std::f64::consts::PI);
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
    let margin = 20.0;
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
  type Reply = UpdatePosition;

  fn handle(&mut self, _message: Tick) -> Self::Reply {
    console::log_1(&format!("🔴 {} processing tick", self.id).into());
    self.move_agent();
    let update = UpdatePosition { agent_id: self.id.clone(), position: self.position.clone() };
    console::log_1(
      &format!("🔴 {} moved to ({:.1}, {:.1})", self.id, self.position.x, self.position.y).into(),
    );
    update
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
}

impl Follower {
  pub fn new(id: String, x: f64, y: f64) -> Self {
    Self {
      id,
      position: Position::new(x, y),
      speed: 0.008,
      follow_distance: 50.0,
      target_leader_id: None,
    }
  }

  fn find_closest_leader(&mut self, leader_positions: &[(String, Position)]) {
    if leader_positions.is_empty() {
      self.target_leader_id = None;
      return;
    }

    let mut closest_distance = f64::INFINITY;
    let mut closest_leader = None;

    for (leader_id, leader_pos) in leader_positions {
      let distance = self.position.distance_to(leader_pos);
      if distance < closest_distance {
        closest_distance = distance;
        closest_leader = Some(leader_id.clone());
      }
    }

    self.target_leader_id = closest_leader.clone();
  }

  fn follow_target(&mut self, leader_positions: &[(String, Position)]) {
    if let Some(target_id) = &self.target_leader_id {
      for (leader_id, leader_pos) in leader_positions {
        if leader_id == target_id {
          let distance = self.position.distance_to(leader_pos);

          if distance > self.follow_distance {
            self.position.move_towards(leader_pos, self.speed);
          }
          break;
        }
      }
    }
  }
}

impl LifeCycle for Follower {}

impl Handler<Tick> for Follower {
  type Reply = UpdatePosition;

  fn handle(&mut self, _message: Tick) -> Self::Reply {
    console::log_1(&format!("🔵 {} processing tick", self.id).into());
    // Since we can't get leader positions reliably yet due to reply routing issues,
    // let's make followers move randomly for now so we can see they're working
    let dx = (random() - 0.5) * self.speed * 2.0;
    let dy = (random() - 0.5) * self.speed * 2.0;

    self.position.x += dx;
    self.position.y += dy;

    // Keep within bounds
    self.position.x = self.position.x.max(10.0).min(790.0);
    self.position.y = self.position.y.max(10.0).min(590.0);

    console::log_1(
      &format!("🔵 {} moved to ({:.1}, {:.1})", self.id, self.position.x, self.position.y).into(),
    );
    UpdatePosition { agent_id: self.id.clone(), position: self.position.clone() }
  }
}
