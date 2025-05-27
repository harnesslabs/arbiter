use arbiter_core::{
  environment::{Database, Middleware},
  error::ArbiterCoreError,
  machine::{Action, Actions, Behavior, ControlFlow, Event, EventStream, Filter},
  messager::{Message, MessageTo, Messager, To},
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TimedMessage {
  delay:           u64,
  receive_data:    String,
  send_data:       String,
  #[serde(skip)]
  messager:        Option<Messager>,
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
    Self { delay, receive_data, send_data, messager: None, count: 0, max_count, startup_message }
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

// TODO: Fix macro
// #[derive(Serialize, Deserialize, Debug)]
// enum Behaviors<DB: Database> {
//   TimedMessage(TimedMessage),
// }
