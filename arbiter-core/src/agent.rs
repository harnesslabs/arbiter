use std::sync::{Arc, Mutex};

pub struct Context<A> {
  agent: Arc<Mutex<A>>,
}

// Default implementation of Context for Arc<Mutex<Agent>>
impl<A> Context<A> {
  pub fn new(agent: A) -> Self { Self { agent: Arc::new(Mutex::new(agent)) } }

  pub fn with<F, R>(&self, f: F) -> R
  where F: FnOnce(&mut A) -> R {
    let mut guard = self.agent.lock().unwrap();
    f(&mut *guard)
  }

  pub fn handle_with<H>(&self, handler: &H, message: H::Message) -> H::Reply
  where H: crate::handler::Handler<A> {
    self.with(|agent| handler.handle(message, agent))
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::handler::Handler;

  pub struct TestAgent {
    state: i32,
  }

  // First handler - increments state using Context
  pub struct IncrementHandler;

  impl Handler<TestAgent> for IncrementHandler {
    type Message = i32;
    type Reply = ();

    fn handle(&self, message: i32, agent: &mut TestAgent) -> () {
      println!("Handler 1 - Received message: {}", message);
      agent.state += message;
      println!("Handler 1 - Updated state: {}", agent.state);
    }
  }

  // Second handler type for different operations
  pub struct MultiplyHandler;

  impl Handler<TestAgent> for MultiplyHandler {
    type Message = i32;
    type Reply = i32;

    fn handle(&self, message: i32, agent: &mut TestAgent) -> i32 {
      println!("Handler 2 - Multiplying state {} by {}", agent.state, message);
      agent.state *= message;
      println!("Handler 2 - Updated state: {}", agent.state);
      agent.state
    }
  }

  #[test]
  fn test_agent_multiple_handlers() {
    let agent = TestAgent { state: 1 };
    let context = Context::new(agent);

    // Use first handler
    let handler1 = IncrementHandler;
    context.handle_with(&handler1, 5);

    // Use second handler on same agent state
    let handler2 = MultiplyHandler;
    let result = context.handle_with(&handler2, 3);
    assert_eq!(result, 18); // (1 + 5) * 3 = 18

    // Verify final state
    context.with(|agent| {
      assert_eq!(agent.state, 18);
    });
  }
}
