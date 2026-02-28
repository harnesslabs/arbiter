#![allow(refining_impl_trait)]

pub mod actor;
pub mod error;
pub mod handler;
pub mod network;
pub mod processor;
pub mod runtime;

pub mod prelude {
  pub use crate::{
    actor::LifeCycle,
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

  #[derive(Debug, Clone)]
  pub struct Counter {
    pub total: i32,
  }

  impl LifeCycle for Counter {
    type Snapshot = i32;
    type StartMessage = ();
    type StopMessage = ();

    fn on_start(&mut self) -> Self::StartMessage {}

    fn on_stop(&mut self) -> Self::StopMessage {}

    fn snapshot(&self) -> Self::Snapshot {
      self.total
    }
  }

  #[derive(Debug, Clone)]
  pub struct Logger {
    pub message_count: i32,
  }

  impl LifeCycle for Logger {
    type Snapshot = i32;
    type StartMessage = ();
    type StopMessage = ();

    fn on_start(&mut self) -> Self::StartMessage {}

    fn on_stop(&mut self) -> Self::StopMessage {}

    fn snapshot(&self) -> Self::Snapshot {
      self.message_count
    }
  }

  impl Handler<NumberMessage> for Counter {
    type Reply = ();

    fn handle(&mut self, message: &NumberMessage) {
      self.total += message.value;
      tracing::debug!(total = self.total, "CounterAgent updated");
    }
  }

  impl Handler<TextMessage> for Logger {
    type Reply = ();

    fn handle(&mut self, message: &TextMessage) {
      self.message_count += 1;
      tracing::debug!(content = %message.content, count = self.message_count, "LogAgent received");
    }
  }

  impl Handler<NumberMessage> for Logger {
    type Reply = ();

    fn handle(&mut self, message: &NumberMessage) {
      self.message_count += 1;
      tracing::debug!(value = message.value, count = self.message_count, "LoggerAgent received");
    }
  }
}
