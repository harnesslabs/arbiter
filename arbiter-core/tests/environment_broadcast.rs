use std::iter::Cycle;

use arbiter_core::{
  agent::LifeCycle,
  environment::Environment,
  network::memory::InMemory,
  prelude::{HandleResult, Handler},
  processor::CreateProcessor,
};

pub struct Clock {
  pub count: usize,
  pub max_count: usize,
}

#[derive(Debug)]
pub struct Proceed;

#[derive(Debug, Clone)]
pub enum TickOrTock {
  Tick,
  Tock,
}

#[derive(Debug, Clone)]
pub struct Stop;

impl LifeCycle for Clock {
  type Snapshot = TickOrTock;
  type StartMessage = TickOrTock;
  type StopMessage = Stop;

  fn on_start(&mut self) -> Self::StartMessage {
    TickOrTock::Tick
  }

  fn on_stop(&mut self) -> Self::StopMessage {
    Stop
  }

  fn snapshot(&self) -> Self::Snapshot {
    if self.count % 2 == 0 { TickOrTock::Tick } else { TickOrTock::Tock }
  }
}

impl Handler<Proceed> for Clock {
  type Reply = TickOrTock;

  fn handle(&mut self, _message: &Proceed) -> HandleResult<TickOrTock> {
    self.count += 1;

    if self.count < self.max_count {
      HandleResult::Message(self.snapshot())
    } else {
      HandleResult::Stop
    }
  }
}

impl Environment for Clock {
  type Instruction = Proceed;

  fn new() -> Self {
    Self { count: 0, max_count: 10 }
  }
}

pub struct Chronos {
  pub message: String,
  pub total_ticks: usize,
  pub total_tocks: usize,
}

impl LifeCycle for Chronos {
  type Snapshot = ();
  type StartMessage = ();
  type StopMessage = ();

  fn on_start(&mut self) -> Self::StartMessage {}

  fn on_stop(&mut self) -> Self::StopMessage {}

  fn snapshot(&self) -> Self::Snapshot {}
}

impl Handler<TickOrTock> for Chronos {
  type Reply = Proceed;

  #[allow(refining_impl_trait)]
  fn handle(&mut self, message: &TickOrTock) -> HandleResult<Self::Reply> {
    match message {
      TickOrTock::Tick => {
        self.total_ticks += 1;
        println!("Chronos received Tick!")
      },
      TickOrTock::Tock => {
        self.total_tocks += 1;
        println!("Chronos received Tock!")
      },
    }
    HandleResult::Message(Proceed)
  }
}

impl Handler<Stop> for Chronos {
  type Reply = ();

  fn handle(&mut self, _message: &Stop) -> HandleResult<Self::Reply> {
    HandleResult::Stop
  }
}

#[tokio::test]
async fn test_environment_process() {
  let runtime = arbiter_core::runtime::Runtime::<InMemory, Clock>::new();
  let mut chronos = runtime
    .spawn(Chronos { message: String::new(), total_ticks: 0, total_tocks: 0 })
    .with_handler::<TickOrTock>()
    .with_handler::<Stop>();
  chronos.set_name("Chronos");
  let mut chronos = chronos.process();
  let mut runtime = runtime.process();
  chronos.start().await;
  runtime.start().await;

  tokio::time::sleep(std::time::Duration::from_millis(10)).await;

  let chronos = chronos.join().await.into_inner();
  let runtime = runtime.join().await;

  assert_eq!(chronos.total_ticks, 5);
  assert_eq!(chronos.total_tocks, 5);
}
