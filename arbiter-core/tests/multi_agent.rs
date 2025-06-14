use arbiter_core::{
  agent::Agent,
  connection::{memory::InMemory, Connection},
  prelude::*,
};

struct PingMessage;

struct PongMessage;

struct StopMessage;

struct Ping {
  pub max_count: usize,
  pub count:     usize,
}

impl LifeCycle for Ping {
  type StartMessage = PingMessage;
  type StopMessage = StopMessage;

  fn on_start(&mut self) -> Self::StartMessage { PingMessage }

  fn on_stop(&mut self) -> Self::StopMessage { StopMessage }
}

impl Handler<PongMessage> for Ping {
  type Reply = PingMessage;

  fn handle(&mut self, message: &PongMessage) -> impl Into<HandleResult<Self::Reply>> {
    if self.count == self.max_count {
      HandleResult::Stop
    } else {
      self.count += 1;
      PingMessage
    }
  }
}

struct Pong;

impl LifeCycle for Pong {
  type StartMessage = ();
  type StopMessage = ();

  fn on_start(&mut self) -> Self::StartMessage {}

  fn on_stop(&mut self) -> Self::StopMessage {}
}

impl Handler<PingMessage> for Pong {
  type Reply = PongMessage;

  fn handle(&mut self, message: &PingMessage) -> Self::Reply { PongMessage }
}

#[test]
fn test_multi_agent() {
  let agent1 = Agent::<Logger, InMemory>::new(Logger {
    name:          "TestLogger".to_string(),
    message_count: 0,
  });
}
