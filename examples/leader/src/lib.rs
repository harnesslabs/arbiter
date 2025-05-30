//! # Leader-Follower Agent Simulation with Arbiter-Core
//!
//! A WebAssembly library that demonstrates a multi-agent system using our custom
//! arbiter-core framework with dynamic agent lifecycle management.

#![cfg(target_arch = "wasm32")]

use std::collections::HashMap;

use arbiter_core::{
  agent::{Agent, AgentContainer, AgentHandler, SharedAgent},
  handler::Handler,
  runtime::Domain,
};
use gloo_timers::future::TimeoutFuture;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
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

impl Agent for Canvas {
  fn on_start(&mut self) { console::log_1(&"Canvas started".into()); }

  fn on_pause(&mut self) { console::log_1(&"Canvas paused".into()); }

  fn on_stop(&mut self) { console::log_1(&"Canvas stopped".into()); }

  fn on_resume(&mut self) { console::log_1(&"Canvas resumed".into()); }
}

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
  domain:         Domain,
}

#[wasm_bindgen]
impl LeaderFollowerSimulation {
  /// Create a new simulation instance
  #[wasm_bindgen(constructor)]
  pub fn new(canvas_width: f64, canvas_height: f64) -> Self {
    console_error_panic_hook::set_once();

    let mut domain = Domain::new();

    // Create and register core agents
    let canvas = Canvas::new(canvas_width, canvas_height);

    {
      let mut runtime = domain.try_lock().unwrap();

      // Register Canvas agent using the new shared agent approach
      console::log_1(&"🎨 Initializing Canvas...".into());
      let shared_canvas = AgentContainer::shared(canvas);

      // Register the shared agent with the runtime
      runtime.register_shared_agent("canvas".to_string(), shared_canvas.clone());

      // Create and register individual handlers for each message type
      let tick_handler = AgentHandler::<Canvas, Tick>::new(shared_canvas.clone());
      let update_position_handler =
        AgentHandler::<Canvas, UpdatePosition>::new(shared_canvas.clone());
      let register_agent_type_handler =
        AgentHandler::<Canvas, RegisterAgentType>::new(shared_canvas.clone());
      let remove_agent_handler = AgentHandler::<Canvas, RemoveAgent>::new(shared_canvas.clone());
      let clear_all_agents_handler =
        AgentHandler::<Canvas, ClearAllAgents>::new(shared_canvas.clone());

      runtime.register_agent_handler(tick_handler);
      runtime.register_agent_handler(update_position_handler);
      runtime.register_agent_handler(register_agent_type_handler);
      runtime.register_agent_handler(remove_agent_handler);
      runtime.register_agent_handler(clear_all_agents_handler);

      runtime.start_agent("canvas");

      runtime.send_message(Tick { leader_positions: Vec::new() });
      runtime.run();
    } // Drop the runtime borrow here

    // Start the automatic tick loop to keep agents moving
    Self::start_tick_loop(domain.clone());

    Self { canvas_width, canvas_height, leader_count: 0, follower_count: 0, domain }
  }

  /// Start the automatic tick loop to drive the simulation
  fn start_tick_loop(domain: Domain) {
    spawn_local(async move {
      loop {
        // Only send ticks if there are agents (other than canvas)
        if let Ok(runtime) = domain.try_lock() {
          let agent_count = runtime.get_agent_stats().total;

          if agent_count > 1 {
            // More than just the canvas
            drop(runtime); // Release the lock before sending message

            if let Ok(mut runtime) = domain.try_lock() {
              runtime.send_message(Tick { leader_positions: Vec::new() });
              runtime.run();
            }
          }
        }

        TimeoutFuture::new(5).await;
      }
    });
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

    let mut runtime = self.domain.try_lock().unwrap();

    let success = match &agent_type {
      AgentType::Leader => {
        let leader = Leader::new(agent_id.clone(), self.canvas_width, self.canvas_height, x, y);

        // Register the leader using SharedAgent approach
        let shared_leader = AgentContainer::shared(leader);
        let registered = runtime.register_shared_agent(agent_id.clone(), shared_leader.clone());

        if registered {
          // Create and register handler for Tick messages
          let tick_handler = AgentHandler::<Leader, Tick>::new(shared_leader);
          runtime.register_agent_handler(tick_handler);

          runtime.start_agent(&agent_id);
          console::log_1(&format!("🔴 {} started", agent_id).into());
        }
        registered
      },
      AgentType::Follower => {
        let follower = Follower::new(agent_id.clone(), x, y);

        // Register the follower using SharedAgent approach
        let shared_follower = AgentContainer::shared(follower);
        let registered = runtime.register_shared_agent(agent_id.clone(), shared_follower.clone());

        if registered {
          // Create and register handler for Tick messages
          let tick_handler = AgentHandler::<Follower, Tick>::new(shared_follower);
          runtime.register_agent_handler(tick_handler);

          runtime.start_agent(&agent_id);
          console::log_1(&format!("🔵 {} started", agent_id).into());
        }
        registered
      },
    };

    if success {
      // Send agent type registration
      runtime.send_message(RegisterAgentType {
        agent_id:   agent_id.clone(),
        agent_type: agent_type.clone(),
      });

      // Send initial position update
      runtime
        .send_message(UpdatePosition { agent_id: agent_id.clone(), position: Position::new(x, y) });

      // Process messages to ensure registration and initial drawing happen
      runtime.run();

      agent_id
    } else {
      console::log_1(&format!("❌ Failed to add {}", agent_id).into());
      String::new()
    }
  }

  /// Get direct access to the runtime domain (for advanced control when wasm feature is enabled)
  #[wasm_bindgen(js_name = "getDomain")]
  pub fn get_domain(&mut self) -> Domain { self.domain.clone() }

  /// Remove an agent and notify Canvas
  #[wasm_bindgen(js_name = "removeAgent")]
  pub fn remove_agent(&mut self, agent_id: &str) -> bool {
    let mut runtime = self.domain.try_lock().unwrap();

    // Remove from domain
    let success = runtime.remove_agent(agent_id);

    if success {
      // Send RemoveAgent message to Canvas
      runtime.send_message(RemoveAgent { agent_id: agent_id.to_string() });

      // Process messages to ensure Canvas gets the removal notification
      runtime.run();

      console::log_1(&format!("🗑️ {} removed", agent_id).into());
    }

    success
  }

  /// Clear all agents and notify Canvas
  #[wasm_bindgen(js_name = "clearAllAgents")]
  pub fn clear_all_agents(&mut self) -> usize {
    let mut runtime = self.domain.try_lock().unwrap();

    // Get list of all agents and filter out canvas
    let all_agents = runtime.list_agents();
    let user_agents: Vec<String> =
      all_agents.into_iter().filter(|id| *id != "canvas").cloned().collect();

    let agent_count = user_agents.len();

    // First, stop all agents
    for agent_id in &user_agents {
      runtime.stop_agent(agent_id);
    }

    // Then remove each agent from runtime
    for agent_id in &user_agents {
      runtime.remove_agent(agent_id);
    }

    // Send a single message to Canvas to clear all visual representations
    runtime.send_message(ClearAllAgents);

    // Reset agent counters
    self.leader_count = 0;
    self.follower_count = 0;

    // Process messages to ensure Canvas gets the clear notification
    runtime.run();

    console::log_1(&"🧹 All agents cleared".into());

    agent_count
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

impl Agent for Leader {
  fn on_start(&mut self) { console::log_1(&format!("Leader {} started", self.id).into()); }

  fn on_pause(&mut self) { console::log_1(&format!("Leader {} paused", self.id).into()); }

  fn on_stop(&mut self) { console::log_1(&format!("Leader {} stopped", self.id).into()); }

  fn on_resume(&mut self) { console::log_1(&format!("Leader {} resumed", self.id).into()); }
}

impl Handler<Tick> for Leader {
  type Reply = UpdatePosition;

  fn handle(&mut self, _message: Tick) -> Self::Reply {
    self.move_agent();
    let update = UpdatePosition { agent_id: self.id.clone(), position: self.position.clone() };
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

impl Agent for Follower {
  fn on_start(&mut self) { console::log_1(&format!("Follower {} started", self.id).into()); }

  fn on_pause(&mut self) { console::log_1(&format!("Follower {} paused", self.id).into()); }

  fn on_stop(&mut self) { console::log_1(&format!("Follower {} stopped", self.id).into()); }

  fn on_resume(&mut self) { console::log_1(&format!("Follower {} resumed", self.id).into()); }
}

impl Handler<Tick> for Follower {
  type Reply = UpdatePosition;

  fn handle(&mut self, message: Tick) -> Self::Reply {
    // Use the leader positions from the tick message
    self.find_closest_leader(&message.leader_positions);
    self.follow_target(&message.leader_positions);

    UpdatePosition { agent_id: self.id.clone(), position: self.position.clone() }
  }
}
