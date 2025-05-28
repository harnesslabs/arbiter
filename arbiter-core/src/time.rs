use std::time::Instant;

use actix::prelude::*;
use chrono::{DateTime, Duration, Utc};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulationTime {
  pub tick:            u64,
  pub simulation_time: DateTime<Utc>,
  pub real_time:       DateTime<Utc>,
  pub tick_duration:   Duration,
}

impl Default for SimulationTime {
  fn default() -> Self {
    Self {
      tick:            0,
      simulation_time: Utc::now(),
      real_time:       Utc::now(),
      tick_duration:   Duration::milliseconds(100), // Default tick duration
    }
  }
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct Tick(pub SimulationTime);

pub struct TimeKeeper {
  time:          SimulationTime,
  subscribers:   Vec<Recipient<Tick>>,
  start_instant: Instant,
}

impl Actor for TimeKeeper {
  type Context = Context<Self>;

  fn started(&mut self, ctx: &mut Context<Self>) {
    // Start the tick interval
    ctx.run_interval(
      std::time::Duration::from_millis(self.time.tick_duration.num_milliseconds() as u64),
      |act, _ctx| {
        act.time.tick += 1;
        act.time.simulation_time = act.time.simulation_time + act.time.tick_duration;
        act.time.real_time = Utc::now();

        // Broadcast tick to all subscribers
        for subscriber in &act.subscribers {
          let _ = subscriber.do_send(Tick(act.time.clone()));
        }
      },
    );
  }
}

impl TimeKeeper {
  pub fn new(tick_duration: Duration) -> Self {
    let mut time = SimulationTime::default();
    time.tick_duration = tick_duration;

    Self { time, subscribers: Vec::new(), start_instant: Instant::now() }
  }

  pub fn current_time(&self) -> SimulationTime { self.time.clone() }

  pub fn subscribe(&mut self, recipient: Recipient<Tick>) { self.subscribers.push(recipient); }
}

#[derive(Debug, Clone)]
pub struct TimeSubscriber {
  pub last_tick: Option<SimulationTime>,
}

impl Actor for TimeSubscriber {
  type Context = Context<Self>;
}

impl Handler<Tick> for TimeSubscriber {
  type Result = ();

  fn handle(&mut self, msg: Tick, _ctx: &mut Context<Self>) { self.last_tick = Some(msg.0); }
}

impl TimeSubscriber {
  pub fn new() -> Self { Self { last_tick: None } }
}
