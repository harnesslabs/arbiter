//! # Leader-Follower Agent Simulation with Arbiter
//!
//! A WebAssembly library that demonstrates a multi-agent system using our custom
//! arbiter framework with dynamic agent lifecycle management.

#![cfg(target_arch = "wasm32")]

pub mod canvas;
pub mod follower;
pub mod leader;

use std::{
  collections::HashMap,
  sync::{Arc, Mutex, OnceLock},
};

use arbiter::{
  actor::LifeCycle,
  handler::{Envelope, Handler},
  network::memory::{InMemory, InMemoryEnvelope},
  runtime::Runtime,
};
use wasm_bindgen::prelude::*;
use web_sys::console;

use crate::{follower::Follower, leader::Leader};

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
  pub const fn new(x: f64, y: f64) -> Self {
    Self { x, y }
  }

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
#[derive(Clone, Copy, Debug)]
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
  console::log_1(&"Leader-Follower Simulation WASM module initialized with Arbiter!".into());
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

#[wasm_bindgen]
pub struct Simulation {
  runtime: Runtime<InMemory>,
}

#[wasm_bindgen]
impl Simulation {
  /// Initialize the leader-follower simulation with shared state
  #[wasm_bindgen(constructor)]
  pub fn new(canvas_width: f64, canvas_height: f64) -> Self {
    console_error_panic_hook::set_once();

    let mut runtime = Runtime::<InMemory>::new();

    unsafe {
      CANVAS_WIDTH = canvas_width;
      CANVAS_HEIGHT = canvas_height;
    }

    // Initialize shared state
    let _shared_state = get_shared_agent_state();

    let canvas = crate::canvas::Canvas::new();
    let canvas_agent = runtime
      .spawn(canvas)
      .with_handler::<crate::canvas::PositionUpdate>()
      .with_handler::<crate::canvas::RemoveAgent>();

    let mut canvas_processing = runtime.process(canvas_agent);

    wasm_bindgen_futures::spawn_local(async move {
      let _ = canvas_processing.start().await;
      Box::leak(Box::new(canvas_processing));
    });

    console::log_1(&"🎨 Shared agent state initialized".into());

    Self { runtime }
  }

  /// Step the simulation forward by one tick
  #[wasm_bindgen]
  pub fn simulation_tick(&mut self) {
    self.runtime.network().send(InMemoryEnvelope::wrap(Tick));
  }

  /// Remove a single agent from shared state
  #[wasm_bindgen]
  pub fn remove_single_agent(&mut self, agent_id: &str) {
    self
      .runtime
      .network()
      .send(InMemoryEnvelope::wrap(crate::canvas::RemoveAgent { id: agent_id.to_string() }));
  }

  /// Clear all agents from shared state and reset counters
  #[wasm_bindgen]
  pub fn clear_all_agents(&mut self) {
    // Clear shared state directly
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

  /// Add an agent at the specified position  
  #[wasm_bindgen]
  pub fn add_agent(&mut self, x: f64, y: f64, is_leader: bool) -> String {
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

    let id_clone = agent_id.clone();

    if is_leader {
      unsafe {
        let leader = Leader::new(agent_id.clone(), CANVAS_WIDTH, CANVAS_HEIGHT, x, y);
        let leader_agent = self.runtime.spawn(leader).with_handler::<Tick>();
        let mut processing = self.runtime.process(leader_agent);
        wasm_bindgen_futures::spawn_local(async move {
          let _ = processing.start().await;
          Box::leak(Box::new(processing));
        });
        console::log_1(&format!("🔴 {agent_id} created and started").into());
      }
    } else {
      let follower = Follower::new(agent_id.clone(), x, y);
      let follower_agent = self.runtime.spawn(follower).with_handler::<Tick>();
      let mut processing = self.runtime.process(follower_agent);
      wasm_bindgen_futures::spawn_local(async move {
        let _ = processing.start().await;
        Box::leak(Box::new(processing));
      });
      console::log_1(&format!("🔵 {agent_id} created and started").into());
    }

    id_clone
  }
}

// Global counters for agent IDs
static mut LEADER_COUNT: u32 = 0;
static mut FOLLOWER_COUNT: u32 = 0;
