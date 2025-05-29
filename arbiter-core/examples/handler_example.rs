use std::collections::HashMap;

use arbiter_core::{
  agent::{AgentBuilder, AgentContext, Handler, Message},
  error::ArbiterCoreError,
  runtime::RuntimeBuilder,
  state_agent::{GetState, SetState, StateAgent},
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// A simple ping message
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Ping {
  message: String,
}

impl Message for Ping {
  type Reply = String;
}

/// A simple ping handler
struct PingHandler;

#[async_trait]
impl Handler<Ping> for PingHandler {
  async fn handle(
    &self,
    msg: Ping,
    _context: &mut AgentContext,
  ) -> Result<String, ArbiterCoreError> {
    Ok(format!("Pong: {}", msg.message))
  }
}

/// A counter message
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Increment {
  amount: i32,
}

impl Message for Increment {
  type Reply = i32;
}

/// A counter handler that maintains state
struct CounterHandler;

#[async_trait]
impl Handler<Increment> for CounterHandler {
  async fn handle(
    &self,
    msg: Increment,
    context: &mut AgentContext,
  ) -> Result<i32, ArbiterCoreError> {
    // Get current count from agent's state
    let current = context.get_state::<i32>("count").copied().unwrap_or(0);
    let new_count = current + msg.amount;

    // Update the state
    context.set_state("count".to_string(), new_count);

    Ok(new_count)
  }
}

#[tokio::main]
async fn main() -> Result<(), ArbiterCoreError> {
  println!("Starting handler-based agent system example...");

  // Create a ping agent
  let (ping_agent, _ping_handle) =
    AgentBuilder::new("ping_agent").with_handler::<Ping, _>(PingHandler).build();

  // Create a counter agent with initial state
  let (counter_agent, counter_handle) = AgentBuilder::new("counter_agent")
    .with_handler::<Increment, _>(CounterHandler)
    .with_state("count".to_string(), 0i32)
    .build();

  // Create a state agent for key-value storage
  let state_agent_handler = StateAgent::<String, String>::new();
  let (state_agent, state_handle) = AgentBuilder::new("state_agent")
    .with_handler::<SetState<String, String>, _>(state_agent_handler.clone())
    .with_handler::<GetState<String>, _>(state_agent_handler)
    .with_state("state".to_string(), HashMap::<String, String>::new())
    .build();

  // Build and start the runtime
  let runtime = RuntimeBuilder::new()
    .with_agent(ping_agent)
    .with_agent(counter_agent)
    .with_agent(state_agent)
    .build_and_run()
    .await?;

  // Test the counter agent
  println!("Testing counter agent...");
  let count1 = counter_handle.send(Increment { amount: 5 }).await?;
  println!("Count after +5: {}", count1);

  let count2 = counter_handle.send(Increment { amount: 3 }).await?;
  println!("Count after +3: {}", count2);

  // Test the state agent
  println!("\nTesting state agent...");
  let set_result = state_handle
    .send(SetState { key: "greeting".to_string(), value: "Hello, World!".to_string() })
    .await?;
  println!("Set result: {:?}", set_result);

  let get_result = state_handle.send(GetState { key: "greeting".to_string() }).await?;
  println!("Get result: {:?}", get_result);

  // List agents in the runtime
  println!("\nRuntime status:");
  println!("Agents running: {}", runtime.agent_count().await);
  println!("Agent IDs: {:?}", runtime.list_agents().await);

  // Shutdown the runtime
  println!("\nShutting down runtime...");
  runtime.shutdown().await?;
  println!("Runtime shutdown complete!");

  Ok(())
}
