use arbiter_core::{
  agent::LifeCycle,
  environment::Environment,
  network::memory::InMemory,
  prelude::{HandleResult, Handler},
};

pub struct Clock {
  pub tick_count: usize,
}

#[derive(Debug)]
pub struct Proceed;

#[derive(Debug)]
pub enum TickOrTock {
  Tick,
  Tock,
}

impl Environment for Clock {
  type State = TickOrTock;
  type Update = Proceed;

  fn new() -> Self {
    Self { tick_count: 0 }
  }

  fn get_state(&self) -> Self::State {
    if self.tick_count % 2 == 0 { TickOrTock::Tick } else { TickOrTock::Tock }
  }

  fn update_state(&mut self, _update: Self::Update) {
    self.tick_count += 1;
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

impl Handler<TickOrTock, Clock> for Chronos {
  type Reply = ();

  #[allow(refining_impl_trait)]
  fn handle(&mut self, message: &TickOrTock) -> HandleResult<Self::Reply, Clock> {
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
    HandleResult::Update(Proceed)
  }
}

#[tokio::test]
async fn test_environment_broadcast() {
  let runtime = arbiter_core::runtime::Runtime::<InMemory, Clock>::new();
  let mut chronos = runtime
    .spawn(Chronos { message: String::new(), total_ticks: 0, total_tocks: 0 })
    .with_handler::<TickOrTock>();
  chronos.set_name("Chronos");
  let mut chronos = chronos.process();

  for _ in 0..5 {
    runtime.broadcast_state().await;
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
  }

  chronos.stop().await;
  let chronos = chronos.join().await.into_inner();

  assert_eq!(chronos.total_ticks, 3);
  assert_eq!(chronos.total_tocks, 2);
}
