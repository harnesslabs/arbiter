use std::sync::{Arc, Mutex};

use arbiter_core::{agent::Agent, network::memory::InMemory, prelude::*};
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

impl Handler<PongMessage> for Ping {
  type Reply = PingMessage;

  #[allow(refining_impl_trait)]
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

  #[allow(refining_impl_trait)]
  fn handle(&mut self, _message: &PingMessage) -> Self::Reply {
    println!("Pong received PingMessage");
    PongMessage
  }
}

#[tokio::test]
async fn test_multi_agent() {
  let network = InMemory::new();

  let mut ping = Agent::<Ping, InMemory>::new_join_network(
    Ping { max_count: 10, count: 0 },
    &network,
    Arc::new(Mutex::new(())),
  )
  .with_handler::<PongMessage>();
  ping.set_name("ping");

  let mut pong =
    Agent::<Pong, InMemory>::new_join_network(Pong, &network, Arc::new(Mutex::new(())))
      .with_handler::<PingMessage>();
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
