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
    SEED = SEED.wrapping_mul(1_103_515_245).wrapping_add(12345);
    f64::from((SEED >> 16) & 0x7fff) / 32767.0
  }
}

/// Position in 2D space
#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// Agent types in our simulation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AgentType {
  Leader,
  Follower,
}

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

/// Message to register an agent type with canvas
#[derive(Clone, Debug)]
pub struct RegisterAgent {
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

/// Canvas agent that manages world state and rendering
#[derive(Clone)]
pub struct Canvas {
  pub canvas_width:  f64,
  pub canvas_height: f64,
  pub agents:        HashMap<String, (AgentType, Position)>,
}

fn get_canvas_context() -> Option<CanvasRenderingContext2d> {
  let window = web_sys::window()?;
  let document = window.document()?;
  let canvas = document.get_element_by_id("canvas")?;
  let canvas: web_sys::HtmlCanvasElement = canvas.dyn_into().ok()?;
  let context = canvas.get_context("2d").ok()??;
  let context: web_sys::CanvasRenderingContext2d = context.dyn_into().ok()?;
  Some(context)
}

impl Canvas {
  pub fn new(canvas_width: f64, canvas_height: f64) -> Self {
    Self { canvas_width, canvas_height, agents: HashMap::new() }
  }

  fn render(&self) {
    let render_id = random() * 1000.0;
    console::log_1(
      &format!("🎨 [Render {}] Canvas rendering {} agents", render_id as u32, self.agents.len())
        .into(),
    );

    let context = match get_canvas_context() {
      Some(ctx) => {
        console::log_1(
          &format!("✅ [Render {}] Successfully got canvas context", render_id as u32).into(),
        );
        ctx
      },
      None => {
        console::log_1(
          &format!("❌ [Render {}] Failed to get canvas context", render_id as u32).into(),
        );
        return;
      },
    };

    // Clear canvas with white background
    console::log_1(&format!("🧹 [Render {}] Clearing canvas...", render_id as u32).into());
    context.set_fill_style(&wasm_bindgen::JsValue::from_str("#ffffff")); // Clean white background
    context.fill_rect(0.0, 0.0, self.canvas_width, self.canvas_height);
    console::log_1(
      &format!("✅ [Render {}] Canvas cleared with white background", render_id as u32).into(),
    );

    // Log all current agents before drawing
    console::log_1(&format!("📍 [Render {}] Current agents:", render_id as u32).into());
    for (agent_id, (agent_type, position)) in &self.agents {
      console::log_1(
        &format!("    {} -> {:?} at ({:.2}, {:.2})", agent_id, agent_type, position.x, position.y)
          .into(),
      );
    }

    // Draw actual agents
    console::log_1(
      &format!("🎯 [Render {}] Drawing {} agents", render_id as u32, self.agents.len()).into(),
    );
    for (agent_id, (agent_type, position)) in &self.agents {
      console::log_1(
        &format!(
          "  🎯 [Render {}] Drawing {} at ({:.2}, {:.2})",
          render_id as u32, agent_id, position.x, position.y
        )
        .into(),
      );

      match agent_type {
        AgentType::Leader => {
          // Draw leader as red circle
          context.set_fill_style(&wasm_bindgen::JsValue::from_str("#dc2626"));
          context.begin_path();
          context.arc(position.x, position.y, 12.0, 0.0, 2.0 * std::f64::consts::PI).unwrap();
          context.fill();
          console::log_1(
            &format!(
              "  ✅ [Render {}] Leader {} drawn at ({:.2}, {:.2})",
              render_id as u32, agent_id, position.x, position.y
            )
            .into(),
          );
        },
        AgentType::Follower => {
          // Draw follower as blue circle
          context.set_fill_style(&wasm_bindgen::JsValue::from_str("#2563eb"));
          context.begin_path();
          context.arc(position.x, position.y, 8.0, 0.0, 2.0 * std::f64::consts::PI).unwrap();
          context.fill();
          console::log_1(
            &format!(
              "  ✅ [Render {}] Follower {} drawn at ({:.2}, {:.2})",
              render_id as u32, agent_id, position.x, position.y
            )
            .into(),
          );
        },
      }
    }

    console::log_1(&format!("🎨 [Render {}] Render complete!", render_id as u32).into());
  }

  fn get_leader_positions(&self) -> Vec<(String, Position)> {
    let leaders: Vec<(String, Position)> = self
      .agents
      .iter()
      .filter_map(|(id, (agent_type, position))| {
        if matches!(agent_type, AgentType::Leader) {
          Some((id.clone(), position.clone()))
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
    console::log_1(
      &format!(
        "🎨 Canvas received position update for {}: ({}, {})",
        message.agent_id, message.position.x, message.position.y
      )
      .into(),
    );

    // Update position while preserving agent type
    if let Some((agent_type, _)) = self.agents.get(&message.agent_id) {
      let agent_type = agent_type.clone();
      self.agents.insert(message.agent_id.clone(), (agent_type, message.position));
      console::log_1(
        &format!("🎨 Updated position for existing agent {}", message.agent_id).into(),
      );
    } else {
      console::log_1(
        &format!("⚠️ Received position update for unknown agent {}", message.agent_id).into(),
      );
    }

    console::log_1(&format!("🎨 Canvas now has {} agents", self.agents.len()).into());
    self.render();
  }
}

impl Handler<RegisterAgent> for Canvas {
  type Reply = ();

  fn handle(&mut self, message: RegisterAgent) -> Self::Reply {
    console::log_1(
      &format!("🎨 Canvas registering agent {} as {:?}", message.agent_id, message.agent_type)
        .into(),
    );

    // Register agent with default position (will be updated later)
    let default_position = Position::new(0.0, 0.0);
    self.agents.insert(message.agent_id, (message.agent_type, default_position));
    console::log_1(
      &format!("🎨 Canvas now has {} agents after registration", self.agents.len()).into(),
    );
    self.render();
  }
}

impl Handler<RemoveAgent> for Canvas {
  type Reply = ();

  fn handle(&mut self, message: RemoveAgent) -> Self::Reply {
    console::log_1(&format!("🎨 Canvas removing agent {}", message.agent_id).into());
    self.agents.remove(&message.agent_id);
    console::log_1(&format!("🎨 Canvas now has {} agents after removal", self.agents.len()).into());
    self.render();
  }
}

impl Handler<ClearAllAgents> for Canvas {
  type Reply = ();

  fn handle(&mut self, _message: ClearAllAgents) -> Self::Reply {
    console::log_1(&"🎨 Canvas clearing all agents".into());

    // Clear all agent data
    self.agents.clear();

    console::log_1(&format!("🎨 Canvas cleared - {} agents remain", self.agents.len()).into());

    // Re-render to show empty canvas
    self.render();
  }
}

impl Handler<Tick> for Canvas {
  type Reply = Tick;

  fn handle(&mut self, message: Tick) -> Self::Reply {
    console::log_1(&"🎨 Canvas received tick, rendering and getting leader positions".into());
    self.render();
    Tick { leader_positions: self.get_leader_positions() }
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
      speed: 0.5,
      current_direction: random() * 2.0 * std::f64::consts::PI,
      direction_steps: 0,
      max_direction_steps: (100 + (random() * 100.0) as u32),
    }
  }

  fn update_direction(&mut self) {
    if self.direction_steps >= self.max_direction_steps {
      let direction_change = (random() - 0.5) * 0.5;
      self.current_direction += direction_change;
      self.current_direction %= (2.0 * std::f64::consts::PI);
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
    console::log_1(&format!("🔴 {} received tick", self.id).into());
    self.move_agent();
    let reply = UpdatePosition { agent_id: self.id.clone(), position: self.position.clone() };
    console::log_1(
      &format!(
        "🔴 {} generating UpdatePosition reply: ({}, {})",
        self.id, reply.position.x, reply.position.y
      )
      .into(),
    );
    reply
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
  pub const fn new(id: String, x: f64, y: f64) -> Self {
    Self {
      id,
      position: Position::new(x, y),
      speed: 0.3,
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

    self.target_leader_id = closest_leader;
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

  fn handle(&mut self, message: Tick) -> Self::Reply {
    console::log_1(&format!("🔵 {} received tick", self.id).into());
    self.find_closest_leader(&message.leader_positions);
    self.follow_target(&message.leader_positions);

    UpdatePosition { agent_id: self.id.clone(), position: self.position.clone() }
  }
}

/// Initialize the leader-follower simulation
#[wasm_bindgen]
pub fn create_leader_follower_simulation(canvas_width: f64, canvas_height: f64) -> Runtime {
  console_error_panic_hook::set_once();

  let mut runtime = Runtime::new();

  // Create and register Canvas agent with all its handlers
  let canvas = Canvas::new(canvas_width, canvas_height);
  let canvas_agent = Agent::new(canvas)
    .with_handler::<UpdatePosition>()
    .with_handler::<RegisterAgent>()
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

  runtime
}

/// Step the simulation forward by one tick
#[wasm_bindgen]
pub fn simulation_tick(runtime: &mut Runtime) {
  console::log_1(
    &format!("=== Starting simulation tick, agent count: {} ===", runtime.agent_count()).into(),
  );

  runtime.step();
}

/// Add an agent at the specified position
#[wasm_bindgen]
pub fn add_simulation_agent(runtime: &mut Runtime, x: f64, y: f64, is_leader: bool) -> String {
  static mut LEADER_COUNT: u32 = 0;
  static mut FOLLOWER_COUNT: u32 = 0;

  let (agent_id, agent_type) = if is_leader {
    unsafe {
      LEADER_COUNT += 1;
      (format!("Leader {LEADER_COUNT}"), AgentType::Leader)
    }
  } else {
    unsafe {
      FOLLOWER_COUNT += 1;
      (format!("Follower {FOLLOWER_COUNT}"), AgentType::Follower)
    }
  };

  let success = match &agent_type {
    AgentType::Leader => {
      let leader = Leader::new(agent_id.clone(), 800.0, 600.0, x, y);
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
    },
    AgentType::Follower => {
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
    },
  };

  if success {
    // Send agent type registration to Canvas
    runtime.broadcast_message(RegisterAgent { agent_id: agent_id.clone(), agent_type });

    agent_id
  } else {
    String::new()
  }
}

/// Test function to manually send UpdatePosition to Canvas
#[wasm_bindgen]
pub fn test_update_position(runtime: &mut Runtime, x: f64, y: f64) {
  console::log_1(&format!("🧪 Testing manual UpdatePosition to Canvas: ({}, {})", x, y).into());

  // First register the test agent as a Leader
  let register_message =
    RegisterAgent { agent_id: "TEST_AGENT".to_string(), agent_type: AgentType::Leader };

  if let Err(e) = runtime.send_to_agent_by_name("canvas", register_message) {
    console::log_1(&format!("❌ Failed to register TEST_AGENT: {}", e).into());
  } else {
    console::log_1(&"✅ RegisterAgent sent to Canvas".into());
  }

  // Then send the position update
  let test_message =
    UpdatePosition { agent_id: "TEST_AGENT".to_string(), position: Position::new(x, y) };

  if let Err(e) = runtime.send_to_agent_by_name("canvas", test_message) {
    console::log_1(&format!("❌ Failed to send UpdatePosition to Canvas: {}", e).into());
  } else {
    console::log_1(&"✅ UpdatePosition sent to Canvas".into());
  }

  // Process the messages
  let processed = runtime.step();
  console::log_1(&format!("🔄 Processed {} messages", processed).into());
}
