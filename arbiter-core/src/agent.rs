use std::{
  any::Any,
  sync::{Arc, Mutex},
};

pub trait Agent: Any {}
// TODO (autoparallel): This could be generalized to handle different kinds of locking. In which
// case we'd make this a trait and have a default implementation that uses Arc<Mutex<A>>.
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

  pub fn handle_with<M>(&self, message: M) -> <A as crate::handler::Handler<M>>::Reply
  where A: crate::handler::Handler<M> {
    self.with(|agent| agent.handle(message))
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::handler::Handler;

  pub struct TestAgent {
    state: i32,
  }

  impl Agent for TestAgent {}

  #[derive(Debug)]
  pub struct Increment(i32);
  #[derive(Debug)]
  pub struct Multiply(i32);

  impl Handler<Increment> for TestAgent {
    type Reply = ();

    fn handle(&mut self, message: Increment) -> Self::Reply {
      println!("Handler 1 - Received message: {:?}", message);
      self.state += message.0;
      println!("Handler 1 - Updated state: {}", self.state);
    }
  }

  impl Handler<Multiply> for TestAgent {
    type Reply = i32;

    fn handle(&mut self, message: Multiply) -> Self::Reply {
      println!("Handler 2 - Multiplying state {} by {}", self.state, message.0);
      self.state *= message.0;
      println!("Handler 2 - Updated state: {}", self.state);
      self.state
    }
  }

  #[test]
  fn test_agent_multiple_handlers() {
    let agent = TestAgent { state: 1 };
    let context = Context::new(agent);

    // Use first handler - agent handles Increment messages
    let increment = Increment(5);
    context.handle_with(increment);

    // Use second handler - agent handles Multiply messages
    let multiply = Multiply(3);
    let result = context.handle_with(multiply);
    assert_eq!(result, 18); // (1 + 5) * 3 = 18

    // Verify final state
    context.with(|agent| {
      assert_eq!(agent.state, 18);
    });
  }
}
