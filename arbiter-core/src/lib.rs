pub mod agent;
pub mod handler;
pub mod network;

pub mod prelude {
  pub use crate::{
    agent::LifeCycle,
    handler::{HandleResult, Handler, Message},
    network::Network,
  };
}

#[cfg(any(test, feature = "fixtures"))]
pub mod fixtures {
  use crate::prelude::*;

  #[derive(Debug, Clone)]
  pub struct NumberMessage {
    pub value: i32,
  }

  #[derive(Debug, Clone)]
  pub struct TextMessage {
    pub content: String,
  }

  pub struct Counter {
    pub total: i32,
  }

  impl LifeCycle for Counter {
    type StartMessage = ();
    type StopMessage = ();

    fn on_start(&mut self) -> Self::StartMessage {}

    fn on_stop(&mut self) -> Self::StopMessage {}
  }

  pub struct Logger {
    pub name:          String,
    pub message_count: i32,
  }

  impl LifeCycle for Logger {
    type StartMessage = ();
    type StopMessage = ();

    fn on_start(&mut self) -> Self::StartMessage {}

    fn on_stop(&mut self) -> Self::StopMessage {}
  }

  impl Handler<NumberMessage> for Counter {
    type Reply = ();

    fn handle(&mut self, message: &NumberMessage) {
      self.total += message.value;
      println!("CounterAgent total is now: {}", self.total);
    }
  }

  impl Handler<TextMessage> for Logger {
    type Reply = ();

    fn handle(&mut self, message: &TextMessage) {
      self.message_count += 1;
      println!(
        "LogAgent '{}' received: '{}' (count: {})",
        self.name, message.content, self.message_count
      );
    }
  }

  impl Handler<NumberMessage> for Logger {
    type Reply = ();

    fn handle(&mut self, message: &NumberMessage) {
      self.message_count += 1;
      println!(
        "LoggerAgent '{}' received: '{}' (count: {})",
        self.name, message.value, self.message_count
      );
    }
  }
}
