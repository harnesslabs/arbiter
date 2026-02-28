use arbiter_core::{network::memory::InMemory, prelude::*, runtime::Runtime};
use tokio_stream::StreamExt;

#[derive(Debug)]
struct PingMessage;

#[derive(Debug)]
struct PongMessage;

#[derive(Debug)]
struct StopMessage;

#[derive(Debug, Clone)]
struct Ping {
  pub max_count: usize,
  pub count:     usize,
}

impl LifeCycle for Ping {
  type Snapshot = usize;
  type StartMessage = PingMessage;
  type StopMessage = StopMessage;

  fn on_start(&mut self) -> Self::StartMessage {
    println!("Ping on_start");
    PingMessage
  }

  fn on_stop(&mut self) -> Self::StopMessage { StopMessage }

  fn snapshot(&self) -> Self::Snapshot { self.count }
}

impl Handler<PongMessage> for Ping {
  type Reply = PingMessage;

  fn handle(&mut self, _message: &PongMessage) -> HandleResult<Self::Reply> {
    println!("Ping received PongMessage, count: {}", self.count);
    if self.count == self.max_count {
      HandleResult::Stop
    } else {
      self.count += 1;
      HandleResult::Message(PingMessage)
    }
  }
}

#[derive(Debug, Clone)]
struct Pong;

impl LifeCycle for Pong {
  type Snapshot = ();
  type StartMessage = ();
  type StopMessage = ();

  fn on_start(&mut self) -> Self::StartMessage {}

  fn on_stop(&mut self) -> Self::StopMessage {}

  fn snapshot(&self) -> Self::Snapshot {}
}

impl Handler<PingMessage> for Pong {
  type Reply = PongMessage;

  fn handle(&mut self, _message: &PingMessage) -> Self::Reply {
    println!("Pong received PingMessage");
    PongMessage
  }
}

#[tokio::test]
async fn test_multi_agent() {
  let runtime = Runtime::<InMemory>::new();

  let mut ping = runtime.spawn(Ping { max_count: 10, count: 0 }).with_handler::<PongMessage>();
  ping.set_name("ping");

  let mut pong = runtime.spawn(Pong).with_handler::<PingMessage>();
  pong.set_name("pong");

  let mut ping = ping.process();
  let mut ping_stream = ping.stream().unwrap();
  ping.start().await.unwrap();

  let mut pong = pong.process();
  pong.start().await.unwrap();

  let agent = ping.join().await.unwrap();
  assert_eq!(agent.inner().count, 10);

  for i in 0..=10 {
    let snapshot = ping_stream.next().await.unwrap();
    println!("`ping` snapshot: {}", snapshot);

    assert_eq!(snapshot, i);
  }

  assert_eq!(ping_stream.next().await, None);
}

pub struct BulletinBoard {
  pub messages: Vec<String>,
}

#[derive(Debug)]
pub struct Write(String);

impl LifeCycle for BulletinBoard {
  type Snapshot = Vec<String>;
  type StartMessage = ();
  type StopMessage = ();

  fn on_start(&mut self) -> Self::StartMessage {}

  fn on_stop(&mut self) -> Self::StopMessage {}

  fn snapshot(&self) -> Self::Snapshot { self.messages.clone() }
}

impl Handler<Write> for BulletinBoard {
  type Reply = ();

  fn handle(&mut self, message: &Write) { self.messages.push(message.0.clone()) }
}

#[derive(Debug)]
pub struct WatchDog {
  pub message_count: usize,
}

impl LifeCycle for WatchDog {
  type Snapshot = ();
  type StartMessage = ();
  type StopMessage = ();

  fn on_start(&mut self) -> Self::StartMessage {}

  fn on_stop(&mut self) -> Self::StopMessage {}

  fn snapshot(&self) -> Self::Snapshot {}
}

impl Handler<PingMessage> for WatchDog {
  type Reply = Write;

  fn handle(&mut self, message: &PingMessage) -> HandleResult<Self::Reply> {
    self.message_count += 1;
    HandleResult::Message(Write(format!(
      "WatchDog observed PingMessage: {:?}, count: {}",
      message, self.message_count
    )))
  }
}

impl Handler<PongMessage> for WatchDog {
  type Reply = Write;

  fn handle(&mut self, message: &PongMessage) -> HandleResult<Self::Reply> {
    self.message_count += 1;
    HandleResult::Message(Write(format!(
      "WatchDog observed PongMessage: {:?}, count: {}",
      message, self.message_count
    )))
  }
}

#[tokio::test]
async fn test_multi_agent_with_coordinator() {
  let runtime = Runtime::<InMemory>::new();

  let mut ping = runtime.spawn(Ping { max_count: 10, count: 0 }).with_handler::<PongMessage>();
  ping.set_name("ping");

  let mut pong = runtime.spawn(Pong).with_handler::<PingMessage>();
  pong.set_name("pong");

  let mut watchdog = runtime
    .spawn(WatchDog { message_count: 0 })
    .with_handler::<PingMessage>()
    .with_handler::<PongMessage>();
  watchdog.set_name("watchdog");

  // BulletinBoard is just a regular agent — no Environment trait needed
  let mut board = runtime
    .spawn(BulletinBoard { messages: vec![] })
    .with_handler::<Write>();
  board.set_name("bulletin_board");

  let mut ping = ping.process();
  let mut pong = pong.process();
  let mut watchdog = watchdog.process();
  let mut board = board.process();

  ping.start().await.unwrap();
  pong.start().await.unwrap();
  watchdog.start().await.unwrap();
  board.start().await.unwrap();

  let ping_agent = ping.join().await.unwrap();
  assert_eq!(ping_agent.inner().count, 10);

  board.stop().await.unwrap();
  let board_core = board.join().await.unwrap();
  let messages = board_core.inner().snapshot();
  println!("BulletinBoard messages: {:?}", messages);
  assert_eq!(messages.len(), 22); // 11 PingMessages and 11 PongMessages
}
