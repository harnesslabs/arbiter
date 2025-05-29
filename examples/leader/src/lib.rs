//! # Leader-Follower Agent Simulation with Arbiter-Core
//!
//! A WebAssembly library that demonstrates a multi-agent system using our custom
//! arbiter-core framework with dynamic agent lifecycle management.

#![cfg(target_arch = "wasm32")]

use std::collections::HashMap;

use arbiter_core::{
  agent::{Agent, AgentState},
  handler::Handler,
  runtime::Domain,
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
      // Clear canvas
      context.clear_rect(0.0, 0.0, self.canvas_width, self.canvas_height);

      // Draw agents
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

      // Draw status
      context.set_fill_style_str("rgba(0, 0, 0, 0.7)");
      context.set_font("12px monospace");
      context.set_text_align("left");
      context
        .fill_text(&format!("🦀 Arbiter-Core - {} agents", self.agent_positions.len()), 10.0, 20.0)
        .unwrap();
    }
  }

  fn get_leader_positions(&self) -> Vec<(String, Position)> {
    console::log_1(
      &format!(
        "Canvas looking for leaders among {} positions and {} types",
        self.agent_positions.len(),
        self.agent_types.len()
      )
      .into(),
    );

    let leaders: Vec<(String, Position)> = self
      .agent_positions
      .iter()
      .filter_map(|(id, pos)| {
        if let Some(agent_type) = self.agent_types.get(id) {
          console::log_1(&format!("Canvas checking agent {}: {:?}", id, agent_type).into());
          if matches!(agent_type, AgentType::Leader) {
            console::log_1(&format!("Canvas found leader {}", id).into());
            Some((id.clone(), pos.clone()))
          } else {
            None
          }
        } else {
          console::log_1(&format!("Canvas has position for {} but no type registered", id).into());
          None
        }
      })
      .collect();

    console::log_1(&format!("Canvas returning {} leader positions", leaders.len()).into());
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
    console::log_1(
      &format!("Canvas registering agent {} as {:?}", message.agent_id, message.agent_type).into(),
    );
    self.agent_types.insert(message.agent_id.clone(), message.agent_type.clone());
    console::log_1(
      &format!("Canvas now has {} agent types registered", self.agent_types.len()).into(),
    );
    self.render();
  }
}

impl Handler<Tick> for Canvas {
  type Reply = Tick;

  fn handle(&mut self, _message: Tick) -> Self::Reply {
    console::log_1(&"Canvas handling tick".into());
    let leader_positions = self.get_leader_positions();
    console::log_1(
      &format!("Canvas tick returning {} leader positions", leader_positions.len()).into(),
    );
    Tick { leader_positions }
  }
}

/// Main simulation structure using arbiter-core
#[wasm_bindgen]
pub struct LeaderFollowerSimulation {
  canvas_width:  f64,
  canvas_height: f64,
  next_id:       u32,
  domain:        Domain,
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
      let mut runtime = domain.as_mut();

      // Add and start core agents
      runtime.add_and_start_agent("canvas".to_string(), canvas.clone());

      // Register handlers for core agents
      runtime.register_handler::<Canvas, Tick>(canvas.clone());
      runtime.register_handler::<Canvas, UpdatePosition>(canvas.clone());
      runtime.register_handler::<Canvas, RegisterAgentType>(canvas);

      runtime.send_message(Tick { leader_positions: Vec::new() });

      runtime.run();
    } // Drop the runtime borrow here

    // Start the automatic tick loop to keep agents moving
    Self::start_tick_loop(domain.clone());

    Self { canvas_width, canvas_height, next_id: 0, domain }
  }

  /// Start the automatic tick loop to drive the simulation
  fn start_tick_loop(domain: Domain) {
    spawn_local(async move {
      loop {
        // Only send ticks if there are agents (other than canvas)
        if let Ok(runtime) = domain.runtime().try_lock() {
          let agent_count = runtime.get_agent_stats().total;

          if agent_count > 1 {
            // More than just the canvas
            drop(runtime); // Release the lock before sending message

            if let Ok(mut runtime) = domain.runtime().try_lock() {
              runtime.send_message(Tick { leader_positions: Vec::new() });
              runtime.run();
            }
          }
        }

        // Wait before next tick (~20 FPS)
        TimeoutFuture::new(10).await;
      }
    });
  }

  /// Add an agent at the specified position (simulation-specific)
  #[wasm_bindgen]
  pub fn add_agent(&mut self, x: f64, y: f64, is_leader: bool) -> String {
    let agent_id = format!("agent_{}", self.next_id);
    self.next_id += 1;

    let agent_type = if is_leader { AgentType::Leader } else { AgentType::Follower };

    let mut runtime = self.domain.as_mut();

    let success = match &agent_type {
      AgentType::Leader => {
        let leader = Leader::new(agent_id.clone(), self.canvas_width, self.canvas_height, x, y);

        // Register the leader and its handlers
        let registered = runtime.add_and_start_agent(agent_id.clone(), leader.clone());
        if registered {
          runtime.register_handler::<Leader, Tick>(leader);
        }
        registered
      },
      AgentType::Follower => {
        let follower = Follower::new(agent_id.clone(), x, y);

        // Register the follower and its handlers
        let registered = runtime.add_and_start_agent(agent_id.clone(), follower.clone());
        if registered {
          runtime.register_handler::<Follower, Tick>(follower);
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

      console::log_1(
        &format!("Added agent {} ({:?}) at ({:.1}, {:.1})", agent_id, agent_type, x, y).into(),
      );

      agent_id
    } else {
      console::log_1(&format!("Failed to add agent {}", agent_id).into());
      String::new()
    }
  }

  /// Get direct access to the runtime domain (for advanced control when wasm feature is enabled)
  #[wasm_bindgen(js_name = "getDomain")]
  pub fn get_domain(&mut self) -> Domain { self.domain.clone() }
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
    UpdatePosition { agent_id: self.id.clone(), position: self.position.clone() }
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
    console::log_1(
      &format!("Follower {} searching through {} leaders", self.id, leader_positions.len()).into(),
    );

    if leader_positions.is_empty() {
      self.target_leader_id = None;
      console::log_1(&format!("Follower {} found no leaders", self.id).into());
      return;
    }

    let mut closest_distance = f64::INFINITY;
    let mut closest_leader = None;

    for (leader_id, leader_pos) in leader_positions {
      let distance = self.position.distance_to(leader_pos);
      console::log_1(
        &format!("Follower {} distance to leader {}: {:.1}", self.id, leader_id, distance).into(),
      );
      if distance < closest_distance {
        closest_distance = distance;
        closest_leader = Some(leader_id.clone());
      }
    }

    self.target_leader_id = closest_leader.clone();
    console::log_1(
      &format!("Follower {} selected target: {:?}", self.id, self.target_leader_id).into(),
    );
  }

  fn follow_target(&mut self, leader_positions: &[(String, Position)]) {
    if let Some(target_id) = &self.target_leader_id {
      console::log_1(&format!("Follower {} attempting to follow {}", self.id, target_id).into());

      for (leader_id, leader_pos) in leader_positions {
        if leader_id == target_id {
          let distance = self.position.distance_to(leader_pos);
          console::log_1(
            &format!(
              "Follower {} distance to target {}: {:.1} (follow_distance: {:.1})",
              self.id, target_id, distance, self.follow_distance
            )
            .into(),
          );

          if distance > self.follow_distance {
            let old_pos = self.position.clone();
            self.position.move_towards(leader_pos, self.speed);
            console::log_1(
              &format!(
                "Follower {} moving from ({:.1}, {:.1}) to ({:.1}, {:.1})",
                self.id, old_pos.x, old_pos.y, self.position.x, self.position.y
              )
              .into(),
            );
          } else {
            console::log_1(
              &format!("Follower {} close enough to leader, not moving", self.id).into(),
            );
          }
          break;
        }
      }
    } else {
      console::log_1(&format!("Follower {} has no target leader", self.id).into());
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
    console::log_1(
      &format!(
        "Follower {} received tick with {} leaders",
        self.id,
        message.leader_positions.len()
      )
      .into(),
    );

    // Use the leader positions from the tick message
    self.find_closest_leader(&message.leader_positions);
    self.follow_target(&message.leader_positions);

    UpdatePosition { agent_id: self.id.clone(), position: self.position.clone() }
  }
}
