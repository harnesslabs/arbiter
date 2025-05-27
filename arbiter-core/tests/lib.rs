use std::collections::HashMap;

use arbiter_core::{
  environment::Database,
  error::ArbiterCoreError,
  machine::{Action, Actions, Behavior, ConfigurableBehavior, ControlFlow, Event, Filter},
  messager::{MessageTo, To},
};
use arbiter_macros::Behaviors;
use serde::{Deserialize, Serialize};

mod engine;

#[derive(Debug, Deserialize, Serialize)]
struct MockBehavior;

// Simple filter that doesn't filter anything
struct NoFilter;

impl<DB: Database> Filter<DB> for NoFilter {
  fn filter(&self, _event: &Event<DB>) -> bool { true }
}

#[async_trait::async_trait]
impl<DB: Database> Behavior<DB> for MockBehavior
where
  DB: Database + 'static,
  DB::Location: Send + Sync + 'static,
  DB::State: Send + Sync + 'static,
{
}

fn default_max_count() -> Option<u64> { Some(3) }

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct TimedMessage {
  delay:           u64,
  receive_data:    String,
  send_data:       String,
  #[serde(default)]
  count:           u64,
  #[serde(default = "default_max_count")]
  max_count:       Option<u64>,
  startup_message: Option<String>,
}

impl TimedMessage {
  #[allow(unused)]
  pub fn new(
    delay: u64,
    receive_data: String,
    send_data: String,
    max_count: Option<u64>,
    startup_message: Option<String>,
  ) -> Self {
    Self { delay, receive_data, send_data, count: 0, max_count, startup_message }
  }
}

// Filter that only passes through Message events
struct MessageFilter;

impl<DB: Database> Filter<DB> for MessageFilter {
  fn filter(&self, event: &Event<DB>) -> bool {
    match event {
      Event::MessageFrom(_) => true,
      Event::StateChange(..) => false,
    }
  }
}

#[async_trait::async_trait]
impl<DB: Database> Behavior<DB> for TimedMessage
where
  DB: Database + 'static,
  DB::Location: Send + Sync + 'static,
  DB::State: Send + Sync + 'static,
{
  fn startup(&mut self) -> Result<(Option<Box<dyn Filter<DB>>>, Actions<DB>), ArbiterCoreError> {
    println!(
      "TimedMessage startup: receive_data={}, send_data={}, startup_message={:?}",
      self.receive_data, self.send_data, self.startup_message
    );

    let mut actions = Actions::new();

    if let Some(startup_message) = &self.startup_message {
      println!("Sending startup message: {}", startup_message);
      actions
        .add_action(Action::MessageTo(MessageTo { to: To::All, data: startup_message.clone() }));
    }

    // TODO: This is really clunky. It would be nice to be able to return just the filter.
    let filter = Some(Box::new(MessageFilter) as Box<dyn Filter<DB>>);

    Ok((filter, actions))
  }

  async fn process_event(
    &mut self,
    event: Event<DB>,
  ) -> Result<(ControlFlow, Actions<DB>), ArbiterCoreError> {
    let mut actions = Actions::new();
    match event {
      Event::MessageFrom(message) =>
        if message.data == self.receive_data {
          println!("Message matches! Sending response: {}", self.send_data);
          let message = MessageTo { to: To::All, data: self.send_data.clone() };
          actions.add_action(Action::MessageTo(message));
          self.count += 1;
          println!("Count incremented to: {}", self.count);
        } else {
          println!("Message does not match, ignoring");
        },
      Event::StateChange(..) => {
        println!("State change, ignoring");
      },
    }

    if self.count == self.max_count.unwrap_or(u64::MAX) {
      println!("Reached max count ({}), halting behavior", self.max_count.unwrap_or(u64::MAX));
      return Ok((ControlFlow::Halt, actions));
    }
    Ok((ControlFlow::Continue, actions))
  }
}

#[derive(Serialize, Deserialize, Debug, Behaviors)]
enum Behaviors {
  TimedMessage(TimedMessage),
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct DatabaseWriter {
  #[serde(default)]
  writes_completed: u8,
  max_writes:       u8,
}

impl DatabaseWriter {
  pub fn new(max_writes: u8) -> Self { Self { writes_completed: 0, max_writes } }
}

// Filter that matches all events so we can process them
struct AllEventsFilter;

impl<DB: Database> Filter<DB> for AllEventsFilter {
  fn filter(&self, _event: &Event<DB>) -> bool { true }
}

#[async_trait::async_trait]
impl Behavior<HashMap<u8, u8>> for DatabaseWriter {
  fn startup(
    &mut self,
  ) -> Result<(Option<Box<dyn Filter<HashMap<u8, u8>>>>, Actions<HashMap<u8, u8>>), ArbiterCoreError>
  {
    println!("DatabaseWriter startup: will write {} times to database", self.max_writes);

    let mut actions = Actions::new();

    // Write the first entry on startup
    actions.add_action(Action::StateChange(self.writes_completed, self.writes_completed * 10));
    self.writes_completed += 1;

    println!(
      "DatabaseWriter: Wrote key={}, value={}",
      self.writes_completed - 1,
      (self.writes_completed - 1) * 10
    );

    // Return a filter that matches all events so we can continue processing
    let filter = Some(Box::new(AllEventsFilter) as Box<dyn Filter<HashMap<u8, u8>>>);

    Ok((filter, actions))
  }

  async fn process_event(
    &mut self,
    _event: Event<HashMap<u8, u8>>,
  ) -> Result<(ControlFlow, Actions<HashMap<u8, u8>>), ArbiterCoreError> {
    let mut actions = Actions::new();

    if self.writes_completed < self.max_writes {
      // Write another entry to the database
      actions.add_action(Action::StateChange(self.writes_completed, self.writes_completed * 10));
      println!(
        "DatabaseWriter: Wrote key={}, value={}",
        self.writes_completed,
        self.writes_completed * 10
      );
      self.writes_completed += 1;

      if self.writes_completed >= self.max_writes {
        println!("DatabaseWriter: Completed {} writes, halting", self.max_writes);
        return Ok((ControlFlow::Halt, actions));
      }

      Ok((ControlFlow::Continue, actions))
    } else {
      println!("DatabaseWriter: Already completed all writes, halting");
      Ok((ControlFlow::Halt, actions))
    }
  }
}
