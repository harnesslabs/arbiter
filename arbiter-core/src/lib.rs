pub mod agent;
// TODO: We need to have a wasm fabric or something.
pub mod connection;
pub mod fabric;
pub mod handler;

pub mod prelude {
  pub use crate::{
    agent::{LifeCycle, RuntimeAgent},
    connection::Connection,
    handler::{Handler, Message},
  };
}

#[cfg(test)]
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

    fn handle(&mut self, message: &NumberMessage) -> Self::Reply {
      self.total += message.value;
      println!("CounterAgent total is now: {}", self.total);
    }
  }

  impl Handler<TextMessage> for Logger {
    type Reply = ();

    fn handle(&mut self, message: &TextMessage) -> Self::Reply {
      self.message_count += 1;
      println!(
        "LogAgent '{}' received: '{}' (count: {})",
        self.name, message.content, self.message_count
      );
    }
  }

  impl Handler<NumberMessage> for Logger {
    type Reply = ();

    fn handle(&mut self, message: &NumberMessage) -> Self::Reply {
      self.message_count += 1;
      println!(
        "LoggerAgent '{}' received: '{}' (count: {})",
        self.name, message.value, self.message_count
      );
    }
  }
}
