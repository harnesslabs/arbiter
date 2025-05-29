use std::sync::{Arc, Mutex};

pub trait Agent {
  fn into_context(self) -> Arc<Mutex<Self>>;
}

pub trait Context {
  fn new() -> Self;
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::handler::Handler;

  pub struct TestAgent {
    state: i32,
  }

  impl Agent for TestAgent {
    fn into_context(self) -> Arc<Mutex<Self>> { Arc::new(Mutex::new(self)) }
  }

  impl Handler<TestAgent> for TestAgent {
    type Message = i32;
    type Reply = ();

    fn handle(&self, message: i32, context: &mut TestAgent) -> () {
      println!("Received message: {}", message);
      context.state = message;
      println!("Updated state: {}", context.state);
    }
  }

  #[test]
  fn test_agent() {
    let agent = TestAgent { state: 0 };
    let context = agent.into_context();
    let mut context = context.lock().unwrap();
    context.state = 1;
    assert_eq!(context.state, 1);
  }
}
