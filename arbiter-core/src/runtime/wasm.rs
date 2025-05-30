//! WASM bindings for the Runtime
//!
//! This module provides JavaScript-friendly wrappers around the core Runtime functionality.
//! All methods return simple types (bool, usize, String) to work well with wasm-bindgen.

// TODO: I still don't think this module is actually necessary, I think we can just stub the
// functions and use the Runtime directly from JS.

use wasm_bindgen::prelude::*;

use super::{Runtime, RuntimeExecutionResult, RuntimeStatistics};
use crate::agent::AgentState;

/// WASM-friendly wrapper around Runtime
#[wasm_bindgen]
pub struct WasmRuntime {
  inner: Runtime,
}

#[wasm_bindgen]
impl WasmRuntime {
  /// Create a new runtime instance
  #[wasm_bindgen(constructor)]
  pub fn new() -> WasmRuntime { WasmRuntime { inner: Runtime::new() } }

  // === CORE EXECUTION ===

  /// Execute a single runtime step
  /// Returns the number of messages processed
  #[wasm_bindgen(js_name = "step")]
  pub fn step(&mut self) -> usize { self.inner.step() }

  /// Run the runtime until stable state or default max steps (1000)
  /// Returns JSON string with execution results
  #[wasm_bindgen(js_name = "run")]
  pub fn run(&mut self) -> String {
    let result = self.inner.run();
    serde_json::to_string(&WasmExecutionResult::from(result)).unwrap_or_else(|_| "{}".to_string())
  }

  /// Run the runtime with a specific step limit
  /// Returns JSON string with execution results
  #[wasm_bindgen(js_name = "runWithLimit")]
  pub fn run_with_limit(&mut self, max_steps: usize) -> String {
    let result = self.inner.run_with_limit(max_steps);
    serde_json::to_string(&WasmExecutionResult::from(result)).unwrap_or_else(|_| "{}".to_string())
  }

  /// Check if the runtime has pending work
  #[wasm_bindgen(js_name = "hasPendingWork")]
  pub fn has_pending_work(&self) -> bool { self.inner.has_pending_work() }

  // === AGENT LIFECYCLE ===

  /// Start an agent by name
  /// Returns true if successful, false if agent not found
  #[wasm_bindgen(js_name = "startAgent")]
  pub fn start_agent(&mut self, agent_name: &str) -> bool {
    self.inner.start_agent_by_name(agent_name).is_ok()
  }

  /// Pause an agent by name
  /// Returns true if successful, false if agent not found
  #[wasm_bindgen(js_name = "pauseAgent")]
  pub fn pause_agent(&mut self, agent_name: &str) -> bool {
    self.inner.pause_agent_by_name(agent_name).is_ok()
  }

  /// Resume an agent by name
  /// Returns true if successful, false if agent not found
  #[wasm_bindgen(js_name = "resumeAgent")]
  pub fn resume_agent(&mut self, agent_name: &str) -> bool {
    self.inner.resume_agent_by_name(agent_name).is_ok()
  }

  /// Stop an agent by name
  /// Returns true if successful, false if agent not found
  #[wasm_bindgen(js_name = "stopAgent")]
  pub fn stop_agent(&mut self, agent_name: &str) -> bool {
    self.inner.stop_agent_by_name(agent_name).is_ok()
  }

  /// Remove an agent by name
  /// Returns true if successful, false if agent not found
  #[wasm_bindgen(js_name = "removeAgent")]
  pub fn remove_agent(&mut self, agent_name: &str) -> bool {
    self.inner.remove_agent_by_name(agent_name).is_ok()
  }

  // === BULK OPERATIONS ===

  /// Start all agents
  /// Returns the number of agents that were started
  #[wasm_bindgen(js_name = "startAllAgents")]
  pub fn start_all_agents(&mut self) -> usize { self.inner.start_all_agents() }

  /// Pause all agents
  /// Returns the number of agents that were paused
  #[wasm_bindgen(js_name = "pauseAllAgents")]
  pub fn pause_all_agents(&mut self) -> usize { self.inner.pause_all_agents() }

  /// Resume all agents
  /// Returns the number of agents that were resumed
  #[wasm_bindgen(js_name = "resumeAllAgents")]
  pub fn resume_all_agents(&mut self) -> usize { self.inner.resume_all_agents() }

  /// Stop all agents
  /// Returns the number of agents that were stopped
  #[wasm_bindgen(js_name = "stopAllAgents")]
  pub fn stop_all_agents(&mut self) -> usize { self.inner.stop_all_agents() }

  /// Remove all agents
  /// Returns the number of agents that were removed
  #[wasm_bindgen(js_name = "removeAllAgents")]
  pub fn remove_all_agents(&mut self) -> usize { self.inner.remove_all_agents().len() }

  // === INFORMATION AND STATISTICS ===

  /// Get the total number of agents
  #[wasm_bindgen(js_name = "agentCount")]
  pub fn agent_count(&self) -> usize { self.inner.agent_count() }

  /// Get the number of agents that need processing
  #[wasm_bindgen(js_name = "agentsNeedingProcessing")]
  pub fn agents_needing_processing(&self) -> usize { self.inner.agents_needing_processing() }

  /// Get agent state by name
  /// Returns "Running", "Paused", "Stopped", or "NotFound"
  #[wasm_bindgen(js_name = "agentState")]
  pub fn agent_state(&self, agent_name: &str) -> String {
    match self.inner.agent_state_by_name(agent_name) {
      Some(AgentState::Running) => "Running".to_string(),
      Some(AgentState::Paused) => "Paused".to_string(),
      Some(AgentState::Stopped) => "Stopped".to_string(),
      None => "NotFound".to_string(),
    }
  }

  /// Look up agent ID by name (returns the raw u64 value)
  /// Returns the agent ID or 0 if not found
  #[wasm_bindgen(js_name = "agentIdByName")]
  pub fn agent_id_by_name(&self, name: &str) -> u64 {
    self.inner.agent_id_by_name(name).map(|id| id.value()).unwrap_or(0)
  }

  /// Get list of all agent names as JSON array
  #[wasm_bindgen(js_name = "agentNames")]
  pub fn agent_names(&self) -> String {
    let names: Vec<&String> = self.inner.agent_names();
    serde_json::to_string(&names).unwrap_or_else(|_| "[]".to_string())
  }

  /// Get list of all agent IDs as JSON array
  #[wasm_bindgen(js_name = "agentIds")]
  pub fn agent_ids(&self) -> String {
    let ids: Vec<u64> = self.inner.agent_ids().iter().map(|id| id.value()).collect();
    serde_json::to_string(&ids).unwrap_or_else(|_| "[]".to_string())
  }

  /// Get runtime statistics as JSON string
  #[wasm_bindgen(js_name = "statistics")]
  pub fn statistics(&self) -> String {
    let stats = self.inner.statistics();
    serde_json::to_string(&WasmStatistics::from(stats)).unwrap_or_else(|_| "{}".to_string())
  }

  /// Get agents by state as JSON array of agent IDs
  #[wasm_bindgen(js_name = "agentsByState")]
  pub fn agents_by_state(&self, state_str: &str) -> String {
    let state = match state_str {
      "Running" => AgentState::Running,
      "Paused" => AgentState::Paused,
      "Stopped" => AgentState::Stopped,
      _ => return "[]".to_string(),
    };

    let agent_ids: Vec<u64> =
      self.inner.agents_by_state(state).iter().map(|id| id.value()).collect();
    serde_json::to_string(&agent_ids).unwrap_or_else(|_| "[]".to_string())
  }

  // === UTILITIES ===

  /// Process all pending messages
  /// Returns the number of messages processed
  #[wasm_bindgen(js_name = "processAllPendingMessages")]
  pub fn process_all_pending_messages(&mut self) -> usize {
    self.inner.process_all_pending_messages()
  }
}

// These methods are for Rust code only, not exported to WASM
impl WasmRuntime {
  /// Get direct access to the inner Runtime (for advanced usage)
  /// This method is not exported to WASM but can be used by Rust code
  pub fn inner(&self) -> &Runtime { &self.inner }

  /// Get mutable access to the inner Runtime (for advanced usage)
  /// This method is not exported to WASM but can be used by Rust code
  pub fn inner_mut(&mut self) -> &mut Runtime { &mut self.inner }
}

// === SERIALIZABLE DATA STRUCTURES ===

/// WASM-friendly version of RuntimeStatistics
#[derive(serde::Serialize)]
struct WasmStatistics {
  total_agents:                 usize,
  running_agents:               usize,
  paused_agents:                usize,
  stopped_agents:               usize,
  agents_with_pending_messages: usize,
}

impl From<RuntimeStatistics> for WasmStatistics {
  fn from(stats: RuntimeStatistics) -> Self {
    Self {
      total_agents:                 stats.total_agents,
      running_agents:               stats.running_agents,
      paused_agents:                stats.paused_agents,
      stopped_agents:               stats.stopped_agents,
      agents_with_pending_messages: stats.agents_with_pending_messages,
    }
  }
}

/// WASM-friendly version of RuntimeExecutionResult
#[derive(serde::Serialize)]
struct WasmExecutionResult {
  total_messages_processed: usize,
  steps_taken:              usize,
  reached_stable_state:     bool,
}

impl From<RuntimeExecutionResult> for WasmExecutionResult {
  fn from(result: RuntimeExecutionResult) -> Self {
    Self {
      total_messages_processed: result.total_messages_processed,
      steps_taken:              result.steps_taken,
      reached_stable_state:     result.reached_stable_state,
    }
  }
}
