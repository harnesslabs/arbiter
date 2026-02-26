use arbiter_core::{
  environment::Environment, network::memory::InMemory, prelude::*, runtime::Runtime,
};
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
  pub count: usize,
}

impl LifeCycle for Ping {
  type Snapshot = usize;
  type StartMessage = PingMessage;
  type StopMessage = StopMessage;

  fn on_start(&mut self) -> Self::StartMessage {
    println!("Ping on_start");
    PingMessage
  }

  fn on_stop(&mut self) -> Self::StopMessage {
    StopMessage
  }

  fn snapshot(&self) -> Self::Snapshot {
    self.count
  }
}

impl<E: Environment> Handler<PongMessage, E> for Ping {
  type Reply = PingMessage;

  #[allow(refining_impl_trait)]
  fn handle(&mut self, _message: &PongMessage) -> HandleResult<Self::Reply, E> {
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

impl<E: Environment> Handler<PingMessage, E> for Pong {
  type Reply = PongMessage;

  #[allow(refining_impl_trait)]
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
  pong.address();

  let mut ping = ping.process();
  let mut ping_stream = ping.stream().await;
  ping.start().await;

  let mut pong = pong.process();
  pong.start().await;

  let agent = ping.join().await;
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

impl Environment for BulletinBoard {
  type State = Vec<String>;
  type Update = String;

  fn new() -> Self {
    Self { messages: vec![] }
  }

  fn get_state(&self) -> Self::State {
    self.messages.clone()
  }

  fn update_state(&mut self, update: Self::Update) {
    self.messages.push(update);
  }
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

impl Handler<PingMessage, BulletinBoard> for WatchDog {
  type Reply = ();

  #[allow(refining_impl_trait)]
  fn handle(&mut self, message: &PingMessage) -> HandleResult<Self::Reply, BulletinBoard> {
    self.message_count += 1;
    HandleResult::Update(format!(
      "WatchDog observed PingMessage: {:?}, count: {}",
      message, self.message_count
    ))
  }
}

impl Handler<PongMessage, BulletinBoard> for WatchDog {
  type Reply = ();

  #[allow(refining_impl_trait)]
  fn handle(&mut self, message: &PongMessage) -> HandleResult<Self::Reply, BulletinBoard> {
    self.message_count += 1;
    HandleResult::Update(format!(
      "WatchDog observed PongMessage: {:?}, count: {}",
      message, self.message_count
    ))
  }
}

#[tokio::test]
async fn test_multi_agent_with_environment() {
  let runtime = Runtime::<InMemory, BulletinBoard>::new();

  let mut ping = runtime.spawn(Ping { max_count: 10, count: 0 }).with_handler::<PongMessage>();
  ping.set_name("ping");

  let mut pong = runtime.spawn(Pong).with_handler::<PingMessage>();
  pong.set_name("pong");

  let mut watchdog = runtime
    .spawn(WatchDog { message_count: 0 })
    .with_handler::<PingMessage>()
    .with_handler::<PongMessage>();
  watchdog.set_name("watchdog");

  let mut ping = ping.process();
  let mut pong = pong.process();
  let mut watchdog = watchdog.process();

  ping.start().await;
  pong.start().await;
  watchdog.start().await;

  let ping_agent = ping.join().await;
  assert_eq!(ping_agent.inner().count, 10);

  let environment = runtime.environment.lock().await;
  let messages = environment.get_state();
  println!("BulletinBoard messages: {:?}", messages);
  assert_eq!(messages.len(), 22); // 11 PingMessages and 11 PongMessages
}
