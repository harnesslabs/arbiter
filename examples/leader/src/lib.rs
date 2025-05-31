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
  pub canvas_width:    f64,
  pub canvas_height:   f64,
  pub agent_positions: HashMap<String, Position>,
  pub agent_types:     HashMap<String, AgentType>,
}

fn get_canvas_context() -> Option<CanvasRenderingContext2d> {
  console::log_1(&"🔍 Getting canvas context...".into());

  let window = web_sys::window()?;
  console::log_1(&"✅ Got window".into());

  let document = window.document()?;
  console::log_1(&"✅ Got document".into());

  let canvas = document.get_element_by_id("canvas")?;
  console::log_1(&"✅ Got canvas element".into());

  let canvas: web_sys::HtmlCanvasElement = canvas.dyn_into().ok()?;
  console::log_1(&"✅ Cast to HtmlCanvasElement".into());

  let context = canvas.get_context("2d").ok()??;
  console::log_1(&"✅ Got 2d context".into());

  let context: web_sys::CanvasRenderingContext2d = context.dyn_into().ok()?;
  console::log_1(&"✅ Cast to CanvasRenderingContext2d - ready to render!".into());

  Some(context)
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

  fn render(&self) {
    console::log_1(&format!("🎨 Canvas rendering {} agents", self.agent_positions.len()).into());

    let context = match get_canvas_context() {
      Some(ctx) => {
        console::log_1(&"✅ Successfully got canvas context".into());
        ctx
      },
      None => {
        console::log_1(&"❌ Failed to get canvas context".into());
        return;
      },
    };

    // Clear canvas with white background
    console::log_1(&"🧹 Clearing canvas...".into());
    context.set_fill_style(&wasm_bindgen::JsValue::from_str("#ffffff")); // Clean white background
    context.fill_rect(0.0, 0.0, self.canvas_width, self.canvas_height);
    console::log_1(&"✅ Canvas cleared with white background".into());

    // Draw actual agents
    console::log_1(&format!("🎯 Drawing {} agents", self.agent_positions.len()).into());
    for (agent_id, position) in &self.agent_positions {
      console::log_1(
        &format!("  🎯 Drawing {} at ({}, {})", agent_id, position.x, position.y).into(),
      );
      let agent_type = self.agent_types.get(agent_id);

      match agent_type {
        Some(AgentType::Leader) => {
          // Draw leader as red circle
          context.set_fill_style(&wasm_bindgen::JsValue::from_str("#dc2626"));
          context.begin_path();
          context.arc(position.x, position.y, 12.0, 0.0, 2.0 * std::f64::consts::PI).unwrap();
          context.fill();
          console::log_1(&format!("  ✅ Leader {} drawn", agent_id).into());
        },
        Some(AgentType::Follower) => {
          // Draw follower as blue circle
          context.set_fill_style(&wasm_bindgen::JsValue::from_str("#2563eb"));
          context.begin_path();
          context.arc(position.x, position.y, 8.0, 0.0, 2.0 * std::f64::consts::PI).unwrap();
          context.fill();
          console::log_1(&format!("  ✅ Follower {} drawn", agent_id).into());
        },
        None => {
          // Unknown agent type - draw as gray circle
          context.set_fill_style(&wasm_bindgen::JsValue::from_str("#6b7280"));
          context.begin_path();
          context.arc(position.x, position.y, 6.0, 0.0, 2.0 * std::f64::consts::PI).unwrap();
          context.fill();
          console::log_1(&format!("  ✅ Unknown agent {} drawn", agent_id).into());
        },
      }
    }

    console::log_1(&"🎨 Render complete!".into());
  }

  fn get_leader_positions(&self) -> Vec<(String, Position)> {
    let leaders: Vec<(String, Position)> = self
      .agent_positions
      .iter()
      .filter_map(|(id, pos)| {
        self.agent_types.get(id).and_then(|agent_type| {
          if matches!(agent_type, AgentType::Leader) {
            Some((id.clone(), pos.clone()))
          } else {
            None
          }
        })
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
    self.agent_positions.insert(message.agent_id, message.position);
    self.render();
  }
}

impl Handler<RegisterAgent> for Canvas {
  type Reply = ();

  fn handle(&mut self, message: RegisterAgent) -> Self::Reply {
    self.agent_types.insert(message.agent_id, message.agent_type);
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

  fn handle(&mut self, message: Tick) -> Self::Reply {
    console::log_1(&"🎨 Canvas received tick, rendering and getting leader positions".into());
    for (leader_id, leader_pos) in message.leader_positions {
      self.agent_positions.insert(leader_id, leader_pos);
    }
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
pub fn simulation_tick(runtime: &mut Runtime) -> usize {
  console::log_1(
    &format!("=== Starting simulation tick, agent count: {} ===", runtime.agent_count()).into(),
  );

  // Step 1: Broadcast tick to all agents (Canvas will render and provide leader positions)
  let delivered = runtime.broadcast_message(Tick { leader_positions: Vec::new() });
  console::log_1(&format!("📡 Tick delivered to {} agents", delivered).into());

  if delivered > 0 {
    // Step 2: Process the tick messages (Canvas renders, agents generate UpdatePosition replies)
    let processed_tick = runtime.step();
    console::log_1(
      &format!("⚡ Step 1 processed {} messages (tick handling)", processed_tick).into(),
    );

    // Check how many agents have pending work after tick processing
    let pending_after_tick = runtime.agents_needing_processing();
    console::log_1(
      &format!("📬 Agents with pending messages after tick: {}", pending_after_tick).into(),
    );

    // Step 3: Process the UpdatePosition replies and route them to Canvas
    let processed_updates = runtime.step();
    console::log_1(
      &format!("⚡ Step 2 processed {} messages (reply routing)", processed_updates).into(),
    );

    // Check again
    let pending_after_routing = runtime.agents_needing_processing();
    console::log_1(
      &format!("📬 Agents with pending messages after routing: {}", pending_after_routing).into(),
    );

    // Step 4: Process any additional routing
    let additional = runtime.step();
    console::log_1(&format!("⚡ Step 3 processed {} messages (additional)", additional).into());

    console::log_1(
      &format!(
        "✅ Total: {} + {} + {} = {}",
        processed_tick,
        processed_updates,
        additional,
        processed_tick + processed_updates + additional
      )
      .into(),
    );

    processed_tick + processed_updates + additional
  } else {
    console::log_1(&"❌ No agents to deliver tick to".into());
    0
  }
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

    // Send initial position update to Canvas
    runtime.broadcast_message(UpdatePosition {
      agent_id: agent_id.clone(),
      position: Position::new(x, y),
    });

    // Process all messages
    runtime.step();

    agent_id
  } else {
    String::new()
  }
}

/// Test function to manually send UpdatePosition to Canvas
#[wasm_bindgen]
pub fn test_update_position(runtime: &mut Runtime, x: f64, y: f64) {
  console::log_1(&format!("🧪 Testing manual UpdatePosition to Canvas: ({}, {})", x, y).into());

  let test_message =
    UpdatePosition { agent_id: "TEST_AGENT".to_string(), position: Position::new(x, y) };

  // Send directly to Canvas
  if let Err(e) = runtime.send_to_agent_by_name("canvas", test_message) {
    console::log_1(&format!("❌ Failed to send to Canvas: {}", e).into());
  } else {
    console::log_1(&"✅ UpdatePosition sent to Canvas".into());
  }

  // Process the message
  let processed = runtime.step();
  console::log_1(&format!("🔄 Processed {} messages", processed).into());
}
