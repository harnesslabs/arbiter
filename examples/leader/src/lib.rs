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
use arbiter::actor::Actor;
use arbiter::processor::Processing;
use std::cell::RefCell;
use std::rc::Rc;

pub enum AgentProcessing {
  Leader(Option<Processing<Actor<Leader, InMemory>, Leader, InMemory>>),
  Follower(Option<Processing<Actor<Follower, InMemory>, Follower, InMemory>>),
}

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
  #[wasm_bindgen(skip)]
  pub agents: HashMap<String, Rc<RefCell<AgentProcessing>>>,
  #[wasm_bindgen(skip)]
  pub agent_states: Rc<RefCell<HashMap<String, String>>>,
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

    Self { runtime, agents: HashMap::new(), agent_states: Rc::new(RefCell::new(HashMap::new())) }
  }

  /// Step the simulation forward by one tick
  #[wasm_bindgen]
  pub fn simulation_tick(&mut self) {
    self.runtime.network().send(InMemoryEnvelope::wrap(Tick));
  }

  /// Remove a single agent from shared state
  #[wasm_bindgen]
  pub fn remove_single_agent(&mut self, agent_id: &str) {
    if let Some(agent) = self.agents.remove(agent_id) {
      wasm_bindgen_futures::spawn_local(async move {
        match &mut *agent.borrow_mut() {
          AgentProcessing::Leader(p) => {
            if let Some(p) = p.take() {
              let _ = p.stop().await;
            }
          },
          AgentProcessing::Follower(p) => {
            if let Some(p) = p.take() {
              let _ = p.stop().await;
            }
          },
        }
      });
    }

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

    // Stop all agents and remove locally
    for (_, agent) in self.agents.drain() {
      wasm_bindgen_futures::spawn_local(async move {
        match &mut *agent.borrow_mut() {
          AgentProcessing::Leader(p) => {
            if let Some(p) = p.take() {
              let _ = p.stop().await;
            }
          },
          AgentProcessing::Follower(p) => {
            if let Some(p) = p.take() {
              let _ = p.stop().await;
            }
          },
        }
      });
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
        let processing =
          Rc::new(RefCell::new(AgentProcessing::Leader(Some(self.runtime.process(leader_agent)))));

        let p_clone = Rc::clone(&processing);
        wasm_bindgen_futures::spawn_local(async move {
          if let AgentProcessing::Leader(Some(p)) = &mut *p_clone.borrow_mut() {
            let _ = p.start().await;
          }
        });

        self.agents.insert(agent_id.clone(), processing);
        self.agent_states.borrow_mut().insert(agent_id.clone(), "Running".to_string());
        console::log_1(&format!("🔴 {agent_id} created and started").into());
      }
    } else {
      let follower = Follower::new(agent_id.clone(), x, y);
      let follower_agent = self.runtime.spawn(follower).with_handler::<Tick>();
      let processing = Rc::new(RefCell::new(AgentProcessing::Follower(Some(
        self.runtime.process(follower_agent),
      ))));

      let p_clone = Rc::clone(&processing);
      wasm_bindgen_futures::spawn_local(async move {
        if let AgentProcessing::Follower(Some(p)) = &mut *p_clone.borrow_mut() {
          let _ = p.start().await;
        }
      });

      self.agents.insert(agent_id.clone(), processing);
      self.agent_states.borrow_mut().insert(agent_id.clone(), "Running".to_string());
      console::log_1(&format!("🔵 {agent_id} created and started").into());
    }

    id_clone
  }

  #[wasm_bindgen(js_name = agentNames)]
  pub fn agent_names(&self) -> String {
    let names: Vec<String> = self.agents.keys().cloned().collect();
    // Serialize manually or use serde_json if available. We can do it manually to avoid adding deps if we want.
    let mut json = String::from("[");
    for (i, name) in names.iter().enumerate() {
      if i > 0 {
        json.push(',');
      }
      json.push_str(&format!("\"{name}\""));
    }
    json.push(']');
    json
  }

  #[wasm_bindgen(js_name = agentState)]
  pub fn agent_state(&self, agent_id: &str) -> String {
    self.agent_states.borrow().get(agent_id).cloned().unwrap_or_else(|| "Unknown".to_string())
  }

  #[wasm_bindgen(js_name = startAgent)]
  pub fn start_agent(&mut self, agent_id: &str) -> bool {
    if let Some(agent) = self.agents.get(agent_id) {
      let agent = Rc::clone(agent);
      let states = Rc::clone(&self.agent_states);
      let id = agent_id.to_string();
      wasm_bindgen_futures::spawn_local(async move {
        match &mut *agent.borrow_mut() {
          AgentProcessing::Leader(Some(p)) => {
            let _ = p.start().await;
          },
          AgentProcessing::Follower(Some(p)) => {
            let _ = p.start().await;
          },
          _ => {},
        }
        states.borrow_mut().insert(id, "Running".to_string());
      });
      true
    } else {
      false
    }
  }

  #[wasm_bindgen(js_name = pauseAgent)]
  pub fn pause_agent(&mut self, agent_id: &str) -> bool {
    if let Some(agent) = self.agents.get(agent_id) {
      let agent = Rc::clone(agent);
      let states = Rc::clone(&self.agent_states);
      let id = agent_id.to_string();
      wasm_bindgen_futures::spawn_local(async move {
        match &mut *agent.borrow_mut() {
          AgentProcessing::Leader(Some(p)) => {
            let _ = p.pause().await;
          },
          AgentProcessing::Follower(Some(p)) => {
            let _ = p.pause().await;
          },
          _ => {},
        }
        states.borrow_mut().insert(id, "Paused".to_string());
      });
      true
    } else {
      false
    }
  }

  #[wasm_bindgen(js_name = stopAgent)]
  pub fn stop_agent(&mut self, agent_id: &str) -> bool {
    if let Some(agent) = self.agents.remove(agent_id) {
      let states = Rc::clone(&self.agent_states);
      let id = agent_id.to_string();
      wasm_bindgen_futures::spawn_local(async move {
        match &mut *agent.borrow_mut() {
          AgentProcessing::Leader(p) => {
            if let Some(p) = p.take() {
              let _ = p.stop().await;
            }
          },
          AgentProcessing::Follower(p) => {
            if let Some(p) = p.take() {
              let _ = p.stop().await;
            }
          },
        }
        states.borrow_mut().insert(id, "Stopped".to_string());
      });
      true
    } else {
      false
    }
  }

  #[wasm_bindgen(js_name = removeAgent)]
  pub fn remove_agent(&mut self, agent_id: &str) -> bool {
    let existed = self.agents.contains_key(agent_id);
    self.remove_single_agent(agent_id);
    self.agent_states.borrow_mut().remove(agent_id);
    existed
  }
}

// Global counters for agent IDs
static mut LEADER_COUNT: u32 = 0;
static mut FOLLOWER_COUNT: u32 = 0;
