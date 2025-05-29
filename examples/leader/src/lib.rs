//! # Leader-Follower Agent Simulation with Arbiter-Core
//!
//! A WebAssembly library that demonstrates a multi-agent system using our custom
//! arbiter-core framework with dynamic agent lifecycle management.

#![cfg(target_arch = "wasm32")]

use std::{
  collections::HashMap,
  sync::{Arc, Mutex},
};

use arbiter_core::{
  agent::{Agent, AgentState},
  handler::Handler,
  runtime::Runtime,
};
use gloo_timers::future::TimeoutFuture;
use js_sys;
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

/// Message to tick the simulation (move agents)
#[derive(Clone, Debug)]
pub struct Tick;

/// Message to get current position
#[derive(Clone, Debug)]
pub struct GetPosition;

/// Reply containing position
#[derive(Clone, Debug)]
pub struct PositionReply(pub Position);

/// Message to render all agents
#[derive(Clone, Debug)]
pub struct RenderAll {
  pub agent_positions: Vec<(String, Position, AgentType)>,
}

/// Message to register an agent type with renderer
#[derive(Clone, Debug)]
pub struct RegisterAgentType {
  pub agent_id:   String,
  pub agent_type: AgentType,
}

/// Message containing leader positions for followers
#[derive(Clone, Debug)]
pub struct LeaderPositions {
  pub positions: Vec<(String, Position)>,
}

// === AGENT IMPLEMENTATIONS ===

/// Leader agent with random movement
#[derive(Clone)]
pub struct LeaderAgent {
  pub id:                  String,
  pub position:            Position,
  pub canvas_width:        f64,
  pub canvas_height:       f64,
  pub speed:               f64,
  pub current_direction:   f64,
  pub direction_steps:     u32,
  pub max_direction_steps: u32,
}

impl LeaderAgent {
  pub fn new(id: String, canvas_width: f64, canvas_height: f64, x: f64, y: f64) -> Self {
    Self {
      id,
      position: Position::new(x, y),
      canvas_width,
      canvas_height,
      speed: 3.0,
      current_direction: random() * 2.0 * std::f64::consts::PI,
      direction_steps: 0,
      max_direction_steps: (30 + (random() * 50.0) as u32),
    }
  }

  fn update_direction(&mut self) {
    if self.direction_steps >= self.max_direction_steps {
      let direction_change = (random() - 0.5) * std::f64::consts::PI;
      self.current_direction += direction_change;
      self.current_direction = self.current_direction % (2.0 * std::f64::consts::PI);
      self.direction_steps = 0;
      self.max_direction_steps = 30 + (random() * 70.0) as u32;
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

impl Agent for LeaderAgent {
  fn on_start(&mut self) { console::log_1(&format!("Leader {} started", self.id).into()); }

  fn on_pause(&mut self) { console::log_1(&format!("Leader {} paused", self.id).into()); }

  fn on_stop(&mut self) { console::log_1(&format!("Leader {} stopped", self.id).into()); }

  fn on_resume(&mut self) { console::log_1(&format!("Leader {} resumed", self.id).into()); }
}

impl Handler<Tick> for LeaderAgent {
  type Reply = UpdatePosition;

  fn handle(&mut self, _message: Tick) -> Self::Reply {
    self.move_agent();
    UpdatePosition { agent_id: self.id.clone(), position: self.position.clone() }
  }
}

impl Handler<GetPosition> for LeaderAgent {
  type Reply = PositionReply;

  fn handle(&mut self, _message: GetPosition) -> Self::Reply {
    PositionReply(self.position.clone())
  }
}

/// Follower agent that follows leaders
#[derive(Clone)]
pub struct FollowerAgent {
  pub id:               String,
  pub position:         Position,
  pub speed:            f64,
  pub follow_distance:  f64,
  pub target_leader_id: Option<String>,
}

impl FollowerAgent {
  pub fn new(id: String, x: f64, y: f64) -> Self {
    Self {
      id,
      position: Position::new(x, y),
      speed: 2.0,
      follow_distance: 40.0,
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

impl Agent for FollowerAgent {
  fn on_start(&mut self) { console::log_1(&format!("Follower {} started", self.id).into()); }

  fn on_pause(&mut self) { console::log_1(&format!("Follower {} paused", self.id).into()); }

  fn on_stop(&mut self) { console::log_1(&format!("Follower {} stopped", self.id).into()); }

  fn on_resume(&mut self) { console::log_1(&format!("Follower {} resumed", self.id).into()); }
}

impl Handler<LeaderPositions> for FollowerAgent {
  type Reply = UpdatePosition;

  fn handle(&mut self, message: LeaderPositions) -> Self::Reply {
    self.find_closest_leader(&message.positions);
    self.follow_target(&message.positions);

    UpdatePosition { agent_id: self.id.clone(), position: self.position.clone() }
  }
}

impl Handler<GetPosition> for FollowerAgent {
  type Reply = PositionReply;

  fn handle(&mut self, _message: GetPosition) -> Self::Reply {
    PositionReply(self.position.clone())
  }
}

/// Renderer agent that manages canvas drawing
#[derive(Clone)]
pub struct RendererAgent {
  pub canvas_width:    f64,
  pub canvas_height:   f64,
  pub agent_positions: HashMap<String, Position>,
  pub agent_types:     HashMap<String, AgentType>,
}

impl RendererAgent {
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
      // Clear canvas
      context.clear_rect(0.0, 0.0, self.canvas_width, self.canvas_height);

      // Draw agents
      for (agent_id, position) in &self.agent_positions {
        let agent_type = self.agent_types.get(agent_id);

        match agent_type {
          Some(AgentType::Leader) => {
            // Draw leader as red circle
            context.set_fill_style(&JsValue::from_str("#ff4444"));
            context.begin_path();
            context.arc(position.x, position.y, 12.0, 0.0, 2.0 * std::f64::consts::PI).unwrap();
            context.fill();
          },
          Some(AgentType::Follower) => {
            // Draw follower as blue circle
            context.set_fill_style(&JsValue::from_str("#4444ff"));
            context.begin_path();
            context.arc(position.x, position.y, 8.0, 0.0, 2.0 * std::f64::consts::PI).unwrap();
            context.fill();
          },
          None => {
            // Unknown agent type, draw as gray
            context.set_fill_style(&JsValue::from_str("#888888"));
            context.begin_path();
            context.arc(position.x, position.y, 6.0, 0.0, 2.0 * std::f64::consts::PI).unwrap();
            context.fill();
          },
        }
      }

      // Draw status
      context.set_fill_style(&JsValue::from_str("rgba(0, 0, 0, 0.7)"));
      context.set_font("12px monospace");
      context.set_text_align("left");
      context
        .fill_text(&format!("🦀 Arbiter-Core - {} agents", self.agent_positions.len()), 10.0, 20.0)
        .unwrap();
    }
  }
}

impl Agent for RendererAgent {
  fn on_start(&mut self) { console::log_1(&"Renderer started".into()); }

  fn on_pause(&mut self) { console::log_1(&"Renderer paused".into()); }

  fn on_stop(&mut self) { console::log_1(&"Renderer stopped".into()); }

  fn on_resume(&mut self) { console::log_1(&"Renderer resumed".into()); }
}

impl Handler<UpdatePosition> for RendererAgent {
  type Reply = ();

  fn handle(&mut self, message: UpdatePosition) -> Self::Reply {
    self.agent_positions.insert(message.agent_id, message.position);
    self.render();
  }
}

impl Handler<RegisterAgentType> for RendererAgent {
  type Reply = ();

  fn handle(&mut self, message: RegisterAgentType) -> Self::Reply {
    console::log_1(
      &format!("Registering agent {} as {:?}", message.agent_id, message.agent_type).into(),
    );
    self.agent_types.insert(message.agent_id, message.agent_type);
    self.render();
  }
}

/// Statistics about the simulation
#[wasm_bindgen]
pub struct SimulationStats {
  leader_count:   usize,
  follower_count: usize,
  total_agents:   usize,
  running_agents: usize,
  paused_agents:  usize,
  stopped_agents: usize,
}

#[wasm_bindgen]
impl SimulationStats {
  #[wasm_bindgen(constructor)]
  pub fn new(
    leader_count: usize,
    follower_count: usize,
    total_agents: usize,
    running_agents: usize,
    paused_agents: usize,
    stopped_agents: usize,
  ) -> Self {
    Self {
      leader_count,
      follower_count,
      total_agents,
      running_agents,
      paused_agents,
      stopped_agents,
    }
  }

  #[wasm_bindgen(getter)]
  pub fn leader_count(&self) -> usize { self.leader_count }

  #[wasm_bindgen(getter)]
  pub fn follower_count(&self) -> usize { self.follower_count }

  #[wasm_bindgen(getter)]
  pub fn total_agents(&self) -> usize { self.total_agents }

  #[wasm_bindgen(getter)]
  pub fn running_agents(&self) -> usize { self.running_agents }

  #[wasm_bindgen(getter)]
  pub fn paused_agents(&self) -> usize { self.paused_agents }

  #[wasm_bindgen(getter)]
  pub fn stopped_agents(&self) -> usize { self.stopped_agents }
}

/// Main simulation structure using arbiter-core
#[wasm_bindgen]
pub struct LeaderFollowerSimulation {
  canvas_width:    f64,
  canvas_height:   f64,
  next_id:         u32,
  runtime:         Arc<Mutex<Runtime>>,
  agents:          HashMap<String, AgentType>,
  agent_positions: Arc<Mutex<HashMap<String, Position>>>, // Shared position tracking
}

#[wasm_bindgen]
impl LeaderFollowerSimulation {
  /// Create a new simulation instance
  #[wasm_bindgen(constructor)]
  pub fn new(canvas_width: f64, canvas_height: f64) -> Self {
    console_error_panic_hook::set_once();

    let mut runtime = Runtime::new().with_max_iterations(1000);

    // Create and register renderer
    let renderer = RendererAgent::new(canvas_width, canvas_height);
    runtime.add_and_start_agent("renderer".to_string(), renderer.clone());

    // Register renderer as handler for the messages it should receive
    runtime.register_handler::<RendererAgent, RegisterAgentType>(renderer.clone());
    runtime.register_handler::<RendererAgent, UpdatePosition>(renderer);

    Self {
      canvas_width,
      canvas_height,
      next_id: 0,
      runtime: Arc::new(Mutex::new(runtime)),
      agents: HashMap::new(),
      agent_positions: Arc::new(Mutex::new(HashMap::new())),
    }
  }

  /// Add an agent at the specified position
  #[wasm_bindgen]
  pub fn add_agent(&mut self, x: f64, y: f64, is_leader: bool) -> String {
    let agent_id = format!("agent_{}", self.next_id);
    self.next_id += 1;

    let agent_type = if is_leader { AgentType::Leader } else { AgentType::Follower };

    let mut runtime = self.runtime.lock().unwrap();

    let success = match &agent_type {
      AgentType::Leader => {
        let leader =
          LeaderAgent::new(agent_id.clone(), self.canvas_width, self.canvas_height, x, y);

        // Register the leader and its handlers
        let registered = runtime.add_and_start_agent(agent_id.clone(), leader.clone());
        if registered {
          runtime.register_handler::<LeaderAgent, Tick>(leader.clone());
          runtime.register_handler::<LeaderAgent, GetPosition>(leader);
        }
        registered
      },
      AgentType::Follower => {
        let follower = FollowerAgent::new(agent_id.clone(), x, y);

        // Register the follower and its handlers
        let registered = runtime.add_and_start_agent(agent_id.clone(), follower.clone());
        if registered {
          runtime.register_handler::<FollowerAgent, LeaderPositions>(follower.clone());
          runtime.register_handler::<FollowerAgent, GetPosition>(follower);
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

      self.agents.insert(agent_id.clone(), agent_type.clone());
      {
        let mut positions = self.agent_positions.lock().unwrap();
        positions.insert(agent_id.clone(), Position::new(x, y));
      }
      console::log_1(
        &format!("Added agent {} ({:?}) at ({:.1}, {:.1})", agent_id, agent_type, x, y).into(),
      );

      // Drop the runtime lock before starting animation
      drop(runtime);

      // Start individual agent animation immediately
      self.start_agent_animation(agent_id.clone(), agent_type);

      agent_id
    } else {
      console::log_1(&format!("Failed to add agent {}", agent_id).into());
      String::new()
    }
  }

  /// Start animation for a specific agent
  fn start_agent_animation(&mut self, agent_id: String, agent_type: AgentType) {
    let runtime_clone = Arc::clone(&self.runtime);
    let agent_positions_clone = Arc::clone(&self.agent_positions);

    console::log_1(&format!("Starting animation for agent {} ({:?})", agent_id, agent_type).into());

    spawn_local(async move {
      loop {
        // Check if agent still exists and is running
        let agent_state = {
          let runtime = runtime_clone.lock().unwrap();
          runtime.get_agent_state(&agent_id)
        }; // Drop the runtime lock immediately

        let should_continue = match agent_state {
          Some(AgentState::Running) => {
            console::log_1(&format!("Agent {} is running", agent_id).into());
            true
          },
          Some(AgentState::Paused) => {
            console::log_1(&format!("Agent {} is paused, sleeping...", agent_id).into());
            // Sleep longer when paused, then continue loop
            TimeoutFuture::new(200).await;
            continue;
          },
          Some(AgentState::Stopped) => {
            console::log_1(&format!("Agent {} is stopped, exiting animation", agent_id).into());
            false
          },
          None => {
            console::log_1(&format!("Agent {} not found, exiting animation", agent_id).into());
            false
          },
        };

        if !should_continue {
          break;
        }

        match agent_type {
          AgentType::Leader => {
            // Update leader position
            let current_time = js_sys::Date::now();
            let time = current_time / 1000.0;

            let new_x = 400.0 + (time * 0.5 + agent_id.len() as f64).sin() * 150.0;
            let new_y = 300.0 + (time * 0.3 + agent_id.len() as f64).cos() * 100.0;
            let new_x = new_x.max(20.0).min(780.0);
            let new_y = new_y.max(20.0).min(580.0);

            let new_pos = Position::new(new_x, new_y);

            // Update position tracking (separate mutex lock)
            {
              let mut positions = agent_positions_clone.lock().unwrap();
              positions.insert(agent_id.clone(), new_pos.clone());
            } // Drop positions lock

            // Send position update to renderer (separate mutex lock)
            {
              let mut runtime = runtime_clone.lock().unwrap();
              runtime
                .send_message(UpdatePosition { agent_id: agent_id.clone(), position: new_pos });
              runtime.run();
            } // Drop runtime lock
          },
          AgentType::Follower => {
            // For followers, we need to get leader positions and follow them
            let leader_positions = {
              let positions = agent_positions_clone.lock().unwrap();
              positions
                .iter()
                .filter(|(id, _)| id.starts_with("agent_") && **id != agent_id)
                .map(|(id, pos)| (id.clone(), pos.clone()))
                .collect::<Vec<_>>()
            }; // Drop positions lock

            if !leader_positions.is_empty() {
              let mut runtime = runtime_clone.lock().unwrap();
              runtime.send_message(LeaderPositions { positions: leader_positions });
              runtime.run();
            } // Drop runtime lock
          },
        }

        // Wait before next frame (~20 FPS)
        TimeoutFuture::new(50).await;
      }

      console::log_1(&format!("Animation stopped for agent {}", agent_id).into());
    });
  }

  /// Start a specific agent
  #[wasm_bindgen]
  pub fn start_agent(&mut self, agent_id: &str) -> bool {
    if let Ok(mut runtime) = self.runtime.try_lock() {
      let success = runtime.start_agent(agent_id);
      if success {
        console::log_1(&format!("Started agent {}", agent_id).into());
      }
      success
    } else {
      console::log_1(&format!("Could not acquire lock to start agent {}", agent_id).into());
      false
    }
  }

  /// Pause a specific agent
  #[wasm_bindgen]
  pub fn pause_agent(&mut self, agent_id: &str) -> bool {
    if let Ok(mut runtime) = self.runtime.try_lock() {
      let success = runtime.pause_agent(agent_id);
      if success {
        console::log_1(&format!("Paused agent {}", agent_id).into());
      }
      success
    } else {
      console::log_1(&format!("Could not acquire lock to pause agent {}", agent_id).into());
      false
    }
  }

  /// Resume a specific agent
  #[wasm_bindgen]
  pub fn resume_agent(&mut self, agent_id: &str) -> bool {
    if let Ok(mut runtime) = self.runtime.try_lock() {
      let success = runtime.resume_agent(agent_id);
      if success {
        console::log_1(&format!("Resumed agent {}", agent_id).into());
      }
      success
    } else {
      console::log_1(&format!("Could not acquire lock to resume agent {}", agent_id).into());
      false
    }
  }

  /// Stop a specific agent
  #[wasm_bindgen]
  pub fn stop_agent(&mut self, agent_id: &str) -> bool {
    if let Ok(mut runtime) = self.runtime.try_lock() {
      let success = runtime.stop_agent(agent_id);
      if success {
        console::log_1(&format!("Stopped agent {}", agent_id).into());
      }
      success
    } else {
      console::log_1(&format!("Could not acquire lock to stop agent {}", agent_id).into());
      false
    }
  }

  /// Remove a specific agent
  #[wasm_bindgen]
  pub fn remove_agent(&mut self, agent_id: &str) -> bool {
    if let Ok(mut runtime) = self.runtime.try_lock() {
      let success = runtime.remove_agent(agent_id);
      if success {
        self.agents.remove(agent_id);
        {
          if let Ok(mut positions) = self.agent_positions.try_lock() {
            positions.remove(agent_id);
          }
        }
        console::log_1(&format!("Removed agent {}", agent_id).into());
      }
      success
    } else {
      console::log_1(&format!("Could not acquire lock to remove agent {}", agent_id).into());
      false
    }
  }

  /// Start all agents
  #[wasm_bindgen]
  pub fn start_all_agents(&mut self) {
    if let Ok(mut runtime) = self.runtime.try_lock() {
      for agent_id in self.agents.keys() {
        runtime.start_agent(agent_id);
      }
      console::log_1(&"Started all agents".into());
    } else {
      console::log_1(&"Could not acquire lock to start all agents".into());
    }
  }

  /// Pause all agents
  #[wasm_bindgen]
  pub fn pause_all_agents(&mut self) {
    if let Ok(mut runtime) = self.runtime.try_lock() {
      for agent_id in self.agents.keys() {
        runtime.pause_agent(agent_id);
      }
      // Don't pause renderer - it should always be active
      console::log_1(&"Paused all agents".into());
    } else {
      console::log_1(&"Could not acquire lock to pause all agents".into());
    }
  }

  /// Resume all agents  
  #[wasm_bindgen]
  pub fn resume_all_agents(&mut self) {
    if let Ok(mut runtime) = self.runtime.try_lock() {
      for agent_id in self.agents.keys() {
        runtime.resume_agent(agent_id);
      }
      // Don't need to resume renderer - it was never paused
      console::log_1(&"Resumed all agents".into());
    } else {
      console::log_1(&"Could not acquire lock to resume all agents".into());
    }
  }

  /// Stop all agents
  #[wasm_bindgen]
  pub fn stop_all_agents(&mut self) {
    if let Ok(mut runtime) = self.runtime.try_lock() {
      for agent_id in self.agents.keys() {
        runtime.stop_agent(agent_id);
      }
      // Don't stop renderer - it should always be active
      console::log_1(&"Stopped all agents".into());
    } else {
      console::log_1(&"Could not acquire lock to stop all agents".into());
    }
  }

  /// Clear all agents and reset simulation
  #[wasm_bindgen]
  pub fn clear_agents(&mut self) {
    if let Ok(mut runtime) = self.runtime.try_lock() {
      for agent_id in self.agents.keys() {
        runtime.remove_agent(agent_id);
      }

      self.agents.clear();
      {
        if let Ok(mut positions) = self.agent_positions.try_lock() {
          positions.clear();
        }
      }
      console::log_1(&"Cleared all agents".into());
    } else {
      console::log_1(&"Could not acquire lock to clear agents".into());
    }
  }

  /// Get simulation statistics
  #[wasm_bindgen]
  pub fn get_stats(&self) -> SimulationStats {
    let leader_count = self.agents.values().filter(|t| matches!(t, AgentType::Leader)).count();
    let follower_count = self.agents.len() - leader_count;

    let mut running_agents = 0;
    let mut paused_agents = 0;
    let mut stopped_agents = 0;

    if let Ok(runtime) = self.runtime.try_lock() {
      for agent_id in self.agents.keys() {
        match runtime.get_agent_state(agent_id) {
          Some(AgentState::Running) => running_agents += 1,
          Some(AgentState::Paused) => paused_agents += 1,
          Some(AgentState::Stopped) => stopped_agents += 1,
          None => stopped_agents += 1, // Treat missing agents as stopped
        }
      }
    } else {
      // If we can't get the lock, treat all agents as stopped
      stopped_agents = self.agents.len();
    }

    SimulationStats::new(
      leader_count,
      follower_count,
      self.agents.len(),
      running_agents,
      paused_agents,
      stopped_agents,
    )
  }

  /// Get the number of agents
  #[wasm_bindgen]
  pub fn agent_count(&self) -> usize { self.agents.len() }

  /// Check if simulation is running
  #[wasm_bindgen]
  pub fn is_running(&self) -> bool {
    if let Ok(runtime) = self.runtime.try_lock() {
      !self.agents.is_empty() && runtime.get_agent_state("renderer").is_some()
    } else {
      false
    }
  }

  /// Get list of agent IDs and their states
  #[wasm_bindgen]
  pub fn get_agent_list(&self) -> String {
    if let Ok(runtime) = self.runtime.try_lock() {
      let mut agents_info = Vec::new();

      for (agent_id, agent_type) in &self.agents {
        let state = runtime.get_agent_state(agent_id).unwrap_or(AgentState::Stopped);
        agents_info.push(format!("{}: {:?} ({:?})", agent_id, agent_type, state));
      }

      agents_info.join("\n")
    } else {
      "Could not acquire lock to get agent list".to_string()
    }
  }

  /// Get agent IDs as a JSON-compatible string
  #[wasm_bindgen]
  pub fn get_agent_ids(&self) -> String {
    let agent_ids: Vec<&String> = self.agents.keys().collect();
    format!("[{}]", agent_ids.iter().map(|id| format!("\"{}\"", id)).collect::<Vec<_>>().join(","))
  }

  /// Get individual agent info as JSON string
  #[wasm_bindgen]
  pub fn get_agent_info(&self, agent_id: &str) -> String {
    if let Ok(runtime) = self.runtime.try_lock() {
      if let Some(agent_type) = self.agents.get(agent_id) {
        let state = runtime.get_agent_state(agent_id).unwrap_or(AgentState::Stopped);
        format!(
          "{{\"id\":\"{}\",\"type\":\"{:?}\",\"state\":\"{:?}\"}}",
          agent_id, agent_type, state
        )
      } else {
        "null".to_string()
      }
    } else {
      // If we can't get the lock, return a "busy" state
      if let Some(agent_type) = self.agents.get(agent_id) {
        format!("{{\"id\":\"{}\",\"type\":\"{:?}\",\"state\":\"Busy\"}}", agent_id, agent_type)
      } else {
        "null".to_string()
      }
    }
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
