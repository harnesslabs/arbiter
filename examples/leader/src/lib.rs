//! # Leader-Follower Agent Simulation with Xtra Actor System
//!
//! A WebAssembly library that demonstrates a multi-agent system using Xtra actors
//! which are designed specifically for WASM compatibility.

#![cfg(target_arch = "wasm32")]

use std::collections::HashMap;

use gloo_timers::future::TimeoutFuture;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use web_sys::{console, CanvasRenderingContext2d, HtmlCanvasElement};
use xtra::{prelude::*, WeakAddress};

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

/// Message to start moving
#[derive(Clone, Debug)]
pub struct StartMoving;

/// Message to update position
#[derive(Clone, Debug)]
pub struct UpdatePosition {
  pub agent_id: String,
  pub position: Position,
}

/// Message to get current position
#[derive(Clone, Debug)]
pub struct GetPosition;

/// Message to tick the simulation
#[derive(Clone, Debug)]
pub struct Tick;

/// Message to stop the timer
#[derive(Clone, Debug)]
pub struct Stop;

/// Message to check if timer is still running
#[derive(Clone, Debug)]
pub struct CheckRunning;

/// Message to register agent type
#[derive(Clone, Debug)]
pub struct RegisterAgentType {
  pub agent_id:   String,
  pub agent_type: AgentType,
}

/// Leader actor with directional movement
#[derive(xtra::Actor)]
pub struct LeaderActor {
  pub id:                  String,
  pub position:            Position,
  pub canvas_width:        f64,
  pub canvas_height:       f64,
  pub speed:               f64,
  pub current_direction:   f64,
  pub direction_steps:     u32,
  pub max_direction_steps: u32,
  pub renderer:            Option<Address<RendererActor>>,
  pub is_moving:           bool,
}

impl LeaderActor {
  pub fn new(
    id: String,
    canvas_width: f64,
    canvas_height: f64,
    renderer: Address<RendererActor>,
  ) -> Self {
    Self {
      id,
      position: Position::new(canvas_width / 2.0, canvas_height / 2.0),
      canvas_width,
      canvas_height,
      speed: 3.0,
      current_direction: random() * 2.0 * std::f64::consts::PI,
      direction_steps: 0,
      max_direction_steps: (30 + (random() * 50.0) as u32),
      renderer: Some(renderer),
      is_moving: false,
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
    if !self.is_moving {
      return;
    }

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

    // Notify renderer
    if let Some(renderer) = &self.renderer {
      let renderer_addr = renderer.clone();
      let agent_id = self.id.clone();
      let position = self.position.clone();
      spawn_local(async move {
        let _ = renderer_addr.send(UpdatePosition { agent_id, position }).await;
      });
    }
  }
}

impl Handler<StartMoving> for LeaderActor {
  type Return = ();

  async fn handle(&mut self, _message: StartMoving, _ctx: &mut Context<Self>) {
    console::log_1(&format!("Leader {} starting movement", self.id).into());
    self.is_moving = true;

    // Send initial position to renderer
    if let Some(renderer) = &self.renderer {
      let renderer_addr = renderer.clone();
      let agent_id = self.id.clone();
      let position = self.position.clone();
      spawn_local(async move {
        let _ = renderer_addr.send(UpdatePosition { agent_id, position }).await;
      });
    }
  }
}

impl Handler<Tick> for LeaderActor {
  type Return = ();

  async fn handle(&mut self, _message: Tick, _ctx: &mut Context<Self>) {
    console::log_1(&format!("Leader {} received tick", self.id).into());
    self.move_agent();
  }
}

impl Handler<GetPosition> for LeaderActor {
  type Return = Position;

  async fn handle(&mut self, _message: GetPosition, _ctx: &mut Context<Self>) -> Position {
    self.position.clone()
  }
}

/// Follower actor that follows the leader
#[derive(xtra::Actor)]
pub struct FollowerActor {
  pub id:              String,
  pub position:        Position,
  pub leader_addr:     Address<LeaderActor>,
  pub speed:           f64,
  pub follow_distance: f64,
  pub renderer:        Option<Address<RendererActor>>,
  pub is_moving:       bool,
}

impl FollowerActor {
  pub fn new(
    id: String,
    leader_addr: Address<LeaderActor>,
    renderer: Address<RendererActor>,
  ) -> Self {
    Self {
      id,
      position: Position::new(50.0 + random() * 200.0, 50.0 + random() * 200.0),
      leader_addr,
      speed: 2.0,
      follow_distance: 40.0,
      renderer: Some(renderer),
      is_moving: false,
    }
  }

  async fn follow_leader(&mut self) {
    if !self.is_moving {
      return;
    }

    if let Ok(leader_pos) = self.leader_addr.send(GetPosition).await {
      let distance = self.position.distance_to(&leader_pos);
      if distance > self.follow_distance {
        self.position.move_towards(&leader_pos, self.speed);

        if let Some(renderer) = &self.renderer {
          let renderer_addr = renderer.clone();
          let agent_id = self.id.clone();
          let position = self.position.clone();
          spawn_local(async move {
            let _ = renderer_addr.send(UpdatePosition { agent_id, position }).await;
          });
        }
      }
    }
  }
}

impl Handler<StartMoving> for FollowerActor {
  type Return = ();

  async fn handle(&mut self, _message: StartMoving, _ctx: &mut Context<Self>) {
    console::log_1(&format!("Follower {} starting to follow leader", self.id).into());
    self.is_moving = true;

    // Send initial position to renderer
    if let Some(renderer) = &self.renderer {
      let renderer_addr = renderer.clone();
      let agent_id = self.id.clone();
      let position = self.position.clone();
      spawn_local(async move {
        let _ = renderer_addr.send(UpdatePosition { agent_id, position }).await;
      });
    }
  }
}

impl Handler<Tick> for FollowerActor {
  type Return = ();

  async fn handle(&mut self, _message: Tick, _ctx: &mut Context<Self>) {
    console::log_1(&format!("Follower {} received tick", self.id).into());
    self.follow_leader().await;
  }
}

/// Renderer actor that draws to canvas
#[derive(xtra::Actor)]
pub struct RendererActor {
  pub canvas_width:    f64,
  pub canvas_height:   f64,
  pub agent_positions: HashMap<String, Position>,
  pub agent_types:     HashMap<String, AgentType>,
}

impl RendererActor {
  pub fn new(canvas_width: f64, canvas_height: f64) -> Self {
    Self {
      canvas_width,
      canvas_height,
      agent_positions: HashMap::new(),
      agent_types: HashMap::new(),
    }
  }

  pub fn set_agent_type(&mut self, agent_id: String, agent_type: AgentType) {
    self.agent_types.insert(agent_id, agent_type);
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
    console::log_1(
      &format!(
        "Rendering {} agents, {} types registered",
        self.agent_positions.len(),
        self.agent_types.len()
      )
      .into(),
    );

    if let Some(context) = self.get_canvas_context() {
      // Clear canvas
      context.clear_rect(0.0, 0.0, self.canvas_width, self.canvas_height);

      // Draw agents
      for (agent_id, position) in &self.agent_positions {
        let agent_type = self.agent_types.get(agent_id);
        console::log_1(
          &format!(
            "Drawing agent {} at ({:.1}, {:.1}) type: {:?}",
            agent_id, position.x, position.y, agent_type
          )
          .into(),
        );

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
            console::log_1(&format!("Warning: Agent {} has no registered type", agent_id).into());
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
        .fill_text(
          &format!("🦀 Xtra Actor System - {} agents", self.agent_positions.len()),
          10.0,
          20.0,
        )
        .unwrap();
    } else {
      console::log_1(&"Failed to get canvas context".into());
    }
  }
}

impl Handler<UpdatePosition> for RendererActor {
  type Return = ();

  async fn handle(&mut self, message: UpdatePosition, _ctx: &mut Context<Self>) {
    self.agent_positions.insert(message.agent_id, message.position);
    self.render();
  }
}

impl Handler<RegisterAgentType> for RendererActor {
  type Return = ();

  async fn handle(&mut self, message: RegisterAgentType, _ctx: &mut Context<Self>) {
    console::log_1(
      &format!("Registering agent {} as {:?}", message.agent_id, message.agent_type).into(),
    );
    self.agent_types.insert(message.agent_id, message.agent_type);
    // Trigger a render in case we have position data already
    self.render();
  }
}

/// Timer actor that sends tick messages
#[derive(xtra::Actor)]
pub struct TimerActor {
  pub leader_targets:   Vec<WeakAddress<LeaderActor>>,
  pub follower_targets: Vec<WeakAddress<FollowerActor>>,
  pub interval_ms:      u32,
  pub is_running:       bool,
}

impl TimerActor {
  pub fn new(interval_ms: u32) -> Self {
    Self {
      leader_targets: Vec::new(),
      follower_targets: Vec::new(),
      interval_ms,
      is_running: false,
    }
  }

  pub fn add_leader_target(&mut self, target: &Address<LeaderActor>) {
    self.leader_targets.push(target.downgrade());
  }

  pub fn add_follower_target(&mut self, target: &Address<FollowerActor>) {
    self.follower_targets.push(target.downgrade());
  }

  async fn start_timer_loop(&self, ctx: &mut Context<Self>) {
    if !self.is_running {
      return;
    }

    let leader_targets = self.leader_targets.clone();
    let follower_targets = self.follower_targets.clone();
    let interval_ms = self.interval_ms;
    let self_addr = ctx.mailbox().address();

    spawn_local(async move {
      loop {
        TimeoutFuture::new(interval_ms).await;

        console::log_1(
          &format!(
            "Timer sending ticks to {} leaders and {} followers",
            leader_targets.len(),
            follower_targets.len()
          )
          .into(),
        );

        // Send tick to all leader targets
        for target in &leader_targets {
          if let Some(target) = target.try_upgrade() {
            let _ = target.send(Tick).await;
          }
        }

        // Send tick to all follower targets
        for target in &follower_targets {
          if let Some(target) = target.try_upgrade() {
            let _ = target.send(Tick).await;
          }
        }

        // Check if we should continue
        if let Ok(is_running) = self_addr.send(CheckRunning).await {
          if !is_running {
            break;
          }
        } else {
          break; // Actor is dead
        }
      }
    });
  }
}

impl Handler<StartMoving> for TimerActor {
  type Return = ();

  async fn handle(&mut self, _message: StartMoving, ctx: &mut Context<Self>) {
    console::log_1(&"Timer starting...".into());
    self.is_running = true;
    self.start_timer_loop(ctx).await;
  }
}

impl Handler<CheckRunning> for TimerActor {
  type Return = bool;

  async fn handle(&mut self, _message: CheckRunning, _ctx: &mut Context<Self>) -> bool {
    self.is_running
  }
}

impl Handler<Stop> for TimerActor {
  type Return = ();

  async fn handle(&mut self, _message: Stop, _ctx: &mut Context<Self>) {
    console::log_1(&"Timer stopping...".into());
    self.is_running = false;
  }
}

/// Statistics about the simulation
#[wasm_bindgen]
pub struct SimulationStats {
  leader_count:   usize,
  follower_count: usize,
  total_agents:   usize,
}

#[wasm_bindgen]
impl SimulationStats {
  #[wasm_bindgen(constructor)]
  pub fn new(leader_count: usize, follower_count: usize, total_agents: usize) -> Self {
    Self { leader_count, follower_count, total_agents }
  }

  #[wasm_bindgen(getter)]
  pub fn leader_count(&self) -> usize { self.leader_count }

  #[wasm_bindgen(getter)]
  pub fn follower_count(&self) -> usize { self.follower_count }

  #[wasm_bindgen(getter)]
  pub fn total_agents(&self) -> usize { self.total_agents }
}

/// Main simulation structure using Xtra actors
#[wasm_bindgen]
pub struct LeaderFollowerSimulation {
  canvas_width:   f64,
  canvas_height:  f64,
  next_id:        u32,
  leader_addr:    Option<Address<LeaderActor>>,
  follower_addrs: Vec<Address<FollowerActor>>,
  renderer_addr:  Address<RendererActor>,
  timer_addr:     Option<Address<TimerActor>>,
  agents:         HashMap<String, AgentType>,
  is_running:     bool,
}

#[wasm_bindgen]
impl LeaderFollowerSimulation {
  /// Create a new simulation instance
  #[wasm_bindgen(constructor)]
  pub fn new(canvas_width: f64, canvas_height: f64) -> Self {
    console_error_panic_hook::set_once();

    // Create renderer actor
    let renderer = RendererActor::new(canvas_width, canvas_height);
    let renderer_addr = xtra::spawn_wasm_bindgen(renderer, Mailbox::unbounded());

    Self {
      canvas_width,
      canvas_height,
      next_id: 0,
      leader_addr: None,
      follower_addrs: Vec::new(),
      renderer_addr,
      timer_addr: None,
      agents: HashMap::new(),
      is_running: false,
    }
  }

  /// Add an agent at the specified position
  #[wasm_bindgen]
  pub fn add_agent(&mut self, x: f64, y: f64, is_leader: bool) -> u32 {
    if self.is_running {
      console::log_1(&"Cannot add agents while simulation is running".into());
      return 0;
    }

    let agent_id = format!("agent_{}", self.next_id);
    let agent_type =
      if is_leader || self.leader_addr.is_none() { AgentType::Leader } else { AgentType::Follower };

    match &agent_type {
      AgentType::Leader => {
        // Create leader actor
        let leader = LeaderActor::new(
          agent_id.clone(),
          self.canvas_width,
          self.canvas_height,
          self.renderer_addr.clone(),
        );
        let leader_addr = xtra::spawn_wasm_bindgen(leader, Mailbox::unbounded());

        // Register agent type with renderer
        console::log_1(&format!("Sending RegisterAgentType for {}", agent_id).into());
        let renderer_addr = self.renderer_addr.clone();
        let register_msg =
          RegisterAgentType { agent_id: agent_id.clone(), agent_type: agent_type.clone() };
        spawn_local(async move {
          let _ = renderer_addr.send(register_msg).await;
        });

        // Update renderer with initial position
        console::log_1(
          &format!("Sending UpdatePosition for {} at ({:.1}, {:.1})", agent_id, x, y).into(),
        );
        let renderer_addr = self.renderer_addr.clone();
        let update_msg =
          UpdatePosition { agent_id: agent_id.clone(), position: Position::new(x, y) };
        spawn_local(async move {
          let _ = renderer_addr.send(update_msg).await;
        });

        self.leader_addr = Some(leader_addr);
      },
      AgentType::Follower => {
        if let Some(leader_addr) = &self.leader_addr {
          // Create follower actor
          let follower =
            FollowerActor::new(agent_id.clone(), leader_addr.clone(), self.renderer_addr.clone());
          let follower_addr = xtra::spawn_wasm_bindgen(follower, Mailbox::unbounded());

          // Register agent type with renderer
          console::log_1(&format!("Sending RegisterAgentType for {}", agent_id).into());
          let renderer_addr = self.renderer_addr.clone();
          let register_msg =
            RegisterAgentType { agent_id: agent_id.clone(), agent_type: agent_type.clone() };
          spawn_local(async move {
            let _ = renderer_addr.send(register_msg).await;
          });

          // Update renderer with initial position
          console::log_1(
            &format!("Sending UpdatePosition for {} at ({:.1}, {:.1})", agent_id, x, y).into(),
          );
          let renderer_addr = self.renderer_addr.clone();
          let update_msg =
            UpdatePosition { agent_id: agent_id.clone(), position: Position::new(x, y) };
          spawn_local(async move {
            let _ = renderer_addr.send(update_msg).await;
          });

          self.follower_addrs.push(follower_addr);
        } else {
          console::log_1(&"Cannot add follower without a leader".into());
          return 0;
        }
      },
    }

    self.agents.insert(agent_id.clone(), agent_type.clone());
    console::log_1(
      &format!("Added agent {} ({:?}) at ({:.1}, {:.1})", agent_id, agent_type, x, y).into(),
    );

    let id = self.next_id;
    self.next_id += 1;
    id
  }

  /// Start the simulation
  #[wasm_bindgen]
  pub fn start_simulation(&mut self) {
    if self.is_running {
      return;
    }

    if self.agents.is_empty() {
      console::log_1(&"No agents to simulate".into());
      return;
    }

    console::log_1(&"Starting Xtra actor simulation...".into());
    self.is_running = true;

    // Create timer actor
    let mut timer = TimerActor::new(50); // 50ms interval

    // Add all agents to timer
    if let Some(leader) = &self.leader_addr {
      timer.add_leader_target(leader);
    }
    for follower in &self.follower_addrs {
      timer.add_follower_target(follower);
    }

    let timer_addr = xtra::spawn_wasm_bindgen(timer, Mailbox::unbounded());
    self.timer_addr = Some(timer_addr.clone());

    // Start leader movement
    if let Some(leader) = &self.leader_addr {
      let leader_addr = leader.clone();
      spawn_local(async move {
        let _ = leader_addr.send(StartMoving).await;
      });
    }

    // Start follower movement
    for follower in &self.follower_addrs {
      let follower_addr = follower.clone();
      spawn_local(async move {
        let _ = follower_addr.send(StartMoving).await;
      });
    }

    // Start timer
    spawn_local(async move {
      let _ = timer_addr.send(StartMoving).await;
    });

    console::log_1(&"Simulation started!".into());
  }

  /// Get simulation statistics
  #[wasm_bindgen]
  pub fn get_stats(&self) -> SimulationStats {
    let leader_count = self.agents.values().filter(|t| matches!(t, AgentType::Leader)).count();
    let follower_count = self.agents.len() - leader_count;
    SimulationStats::new(leader_count, follower_count, self.agents.len())
  }

  /// Clear all agents and reset simulation
  #[wasm_bindgen]
  pub fn clear_agents(&mut self) {
    if self.is_running {
      console::log_1(&"Cannot clear agents while simulation is running".into());
      return;
    }

    // Stop timer
    if let Some(timer) = &self.timer_addr {
      let _ = timer.send(Stop);
    }

    // Actors will be automatically cleaned up when addresses are dropped
    self.leader_addr = None;
    self.follower_addrs.clear();
    self.timer_addr = None;
    self.agents.clear();
    self.is_running = false;

    console::log_1(&"Cleared all agents".into());
  }

  /// Get the number of agents
  #[wasm_bindgen]
  pub fn agent_count(&self) -> usize { self.agents.len() }

  /// Check if simulation is running
  #[wasm_bindgen]
  pub fn is_running(&self) -> bool { self.is_running }
}

/// Initialize the WASM module
#[wasm_bindgen(start)]
pub fn main() {
  console_error_panic_hook::set_once();
  console::log_1(&"🦀 Leader-Follower Simulation WASM module initialized with Xtra!".into());
}
