#![cfg(all(feature = "wasm", target_arch = "wasm32"))]

use wasm_bindgen::prelude::*;

use super::*;

#[wasm_bindgen]
impl Domain {
  /// Create a new Domain for WASM
  #[wasm_bindgen(constructor)]
  pub fn new_js() -> Domain { Domain::new() }

  /// Get agent state as string
  #[wasm_bindgen(js_name = "getAgentState")]
  pub fn get_agent_state_js(&self, agent_id: &str) -> String {
    if let Ok(runtime) = self.runtime.try_lock() {
      match runtime.get_agent_state(agent_id) {
        Some(state) => format!("{:?}", state),
        None => "NotFound".to_string(),
      }
    } else {
      "Locked".to_string()
    }
  }

  /// Start an agent
  #[wasm_bindgen(js_name = "startAgent")]
  pub fn start_agent_js(&mut self, agent_id: &str) -> bool {
    let mut runtime = self.as_mut();
    runtime.start_agent(agent_id)
  }

  /// Pause an agent
  #[wasm_bindgen(js_name = "pauseAgent")]
  pub fn pause_agent_js(&mut self, agent_id: &str) -> bool {
    let mut runtime = self.as_mut();
    runtime.pause_agent(agent_id)
  }

  /// Stop an agent
  #[wasm_bindgen(js_name = "stopAgent")]
  pub fn stop_agent_js(&mut self, agent_id: &str) -> bool {
    let mut runtime = self.as_mut();
    runtime.stop_agent(agent_id)
  }

  /// Resume an agent
  #[wasm_bindgen(js_name = "resumeAgent")]
  pub fn resume_agent_js(&mut self, agent_id: &str) -> bool {
    let mut runtime = self.as_mut();
    runtime.resume_agent(agent_id)
  }

  /// Run the runtime (process messages)
  #[wasm_bindgen(js_name = "run")]
  pub fn run_js(&mut self) -> usize {
    let mut runtime = self.as_mut();
    runtime.run()
  }

  /// Get queue length
  #[wasm_bindgen(js_name = "queueLength")]
  pub fn queue_length_js(&self) -> usize {
    if let Ok(runtime) = self.runtime.try_lock() {
      runtime.queue_length()
    } else {
      0
    }
  }

  /// Check if runtime is idle
  #[wasm_bindgen(js_name = "isIdle")]
  pub fn is_idle_js(&self) -> bool {
    if let Ok(runtime) = self.runtime.try_lock() {
      runtime.is_idle()
    } else {
      false
    }
  }

  /// List all agent names as JSON array
  #[wasm_bindgen(js_name = "listAgents")]
  pub fn list_agents_js(&self) -> String {
    if let Ok(runtime) = self.runtime.try_lock() {
      let agents = runtime.list_agents();
      serde_json::to_string(&agents).unwrap_or_else(|_| "[]".to_string())
    } else {
      "[]".to_string()
    }
  }

  /// Remove an agent
  #[wasm_bindgen(js_name = "removeAgent")]
  pub fn remove_agent_js(&mut self, agent_id: &str) -> bool {
    let mut runtime = self.as_mut();
    runtime.remove_agent(agent_id)
  }

  /// Start all agents
  #[wasm_bindgen(js_name = "startAllAgents")]
  pub fn start_all_agents_js(&mut self) -> usize {
    let mut runtime = self.as_mut();
    runtime.start_all_agents()
  }

  /// Pause all agents
  #[wasm_bindgen(js_name = "pauseAllAgents")]
  pub fn pause_all_agents_js(&mut self) -> usize {
    let mut runtime = self.as_mut();
    runtime.pause_all_agents()
  }

  /// Resume all agents
  #[wasm_bindgen(js_name = "resumeAllAgents")]
  pub fn resume_all_agents_js(&mut self) -> usize {
    let mut runtime = self.as_mut();
    runtime.resume_all_agents()
  }

  /// Stop all agents
  #[wasm_bindgen(js_name = "stopAllAgents")]
  pub fn stop_all_agents_js(&mut self) -> usize {
    let mut runtime = self.as_mut();
    runtime.stop_all_agents()
  }

  /// Clear all agents
  #[wasm_bindgen(js_name = "clearAllAgents")]
  pub fn clear_all_agents_js(&mut self) -> usize {
    let mut runtime = self.as_mut();
    runtime.clear_all_agents()
  }

  /// Get agents by state as JSON array
  #[wasm_bindgen(js_name = "getAgentsByState")]
  pub fn get_agents_by_state_js(&self, state_str: &str) -> String {
    if let Ok(runtime) = self.runtime.try_lock() {
      let state = match state_str {
        "Running" => crate::agent::AgentState::Running,
        "Paused" => crate::agent::AgentState::Paused,
        "Stopped" => crate::agent::AgentState::Stopped,
        _ => return "[]".to_string(),
      };
      let agents = runtime.get_agents_by_state(state);
      serde_json::to_string(&agents).unwrap_or_else(|_| "[]".to_string())
    } else {
      "[]".to_string()
    }
  }

  /// Get agent statistics as JSON
  #[wasm_bindgen(js_name = "getAgentStats")]
  pub fn get_agent_stats_js(&self) -> String {
    if let Ok(runtime) = self.runtime.try_lock() {
      let stats = runtime.get_agent_stats();
      format!(
        "{{\"total\":{},\"running\":{},\"paused\":{},\"stopped\":{}}}",
        stats.total, stats.running, stats.paused, stats.stopped
      )
    } else {
      "{\"total\":0,\"running\":0,\"paused\":0,\"stopped\":0}".to_string()
    }
  }
}
