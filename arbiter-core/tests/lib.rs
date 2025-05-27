use arbiter_core::{
  environment::{Database, Middleware},
  error::ArbiterCoreError,
  machine::{
    Actions, Behavior, ControlFlow, CreateEngine, Engine, EngineType, EventStream, Filter,
    UnifiedEvent,
  },
  messager::{Message, Messager, To},
};
use arbiter_macros::Behaviors;
use serde::{Deserialize, Serialize};

mod engine;

#[derive(Debug, Deserialize, Serialize)]
struct MockBehavior;

// Simple filter that doesn't filter anything
struct NoFilter;

impl<DB: Database> Filter<DB, ()> for NoFilter {
  fn filter(&self, _event: UnifiedEvent<DB>) -> Option<()> { Some(()) }
}

#[async_trait::async_trait]
impl<DB: Database> Behavior<DB, ()> for MockBehavior
where
  DB::Location: Send + Sync + 'static,
  DB::State: Send + Sync + 'static,
{
  type Filter = NoFilter;

  async fn startup(
    &mut self,
    _middleware: Middleware<DB>,
    _messager: Messager,
  ) -> Result<(Option<Self::Filter>, Option<Actions<DB>>), ArbiterCoreError> {
    Ok((None, None))
  }
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

impl<DB: Database> Filter<DB, Message> for MessageFilter {
  fn filter(&self, event: UnifiedEvent<DB>) -> Option<Message> {
    match event {
      UnifiedEvent::Message(msg) => Some(msg),
      UnifiedEvent::StateChange(..) => None,
    }
  }
}

#[async_trait::async_trait]
impl<DB: Database> Behavior<DB, Message> for TimedMessage
where
  DB::Location: Send + Sync + 'static,
  DB::State: Send + Sync + 'static,
{
  type Filter = MessageFilter;

  async fn startup(
    &mut self,
    _middleware: Middleware<DB>,
    messager: Messager,
  ) -> Result<(Option<Self::Filter>, Option<Actions<DB>>), ArbiterCoreError> {
    println!(
      "TimedMessage startup: receive_data={}, send_data={}, startup_message={:?}",
      self.receive_data, self.send_data, self.startup_message
    );

    let mut actions = Actions::new();

    if let Some(startup_message) = &self.startup_message {
      println!("Sending startup message: {}", startup_message);
      let message = Message {
        from: messager.id.clone().unwrap_or_default(),
        to:   To::All,
        data: serde_json::to_string(startup_message).unwrap(),
      };
      actions.add_action(arbiter_core::machine::Action::Message(message));
    }

    self.messager = Some(messager);

    let filter = Some(MessageFilter);
    let actions = if actions.get_actions().is_empty() { None } else { Some(actions) };

    Ok((filter, actions))
  }

  async fn process_event(
    &mut self,
    event: Message,
  ) -> Result<(ControlFlow, Option<Actions<DB>>), ArbiterCoreError> {
    println!(
      "TimedMessage process: received message from={}, data={}, looking_for={}",
      event.from,
      event.data,
      serde_json::to_string(&self.receive_data).unwrap()
    );

    let mut actions = Actions::new();

    if event.data == serde_json::to_string(&self.receive_data).unwrap() {
      println!("Message matches! Sending response: {}", self.send_data);
      let message = Message {
        from: self.messager.as_ref().unwrap().id.clone().unwrap_or_default(),
        to:   To::All,
        data: serde_json::to_string(&self.send_data).unwrap(),
      };
      actions.add_action(arbiter_core::machine::Action::Message(message));
      self.count += 1;
      println!("Count incremented to: {}", self.count);
    } else {
      println!("Message does not match, ignoring");
    }

    let actions = if actions.get_actions().is_empty() { None } else { Some(actions) };

    if self.count == self.max_count.unwrap_or(u64::MAX) {
      println!("Reached max count ({}), halting behavior", self.max_count.unwrap_or(u64::MAX));
      return Ok((ControlFlow::Halt, actions));
    }
    Ok((ControlFlow::Continue, actions))
  }
}

#[derive(Serialize, Deserialize, Debug)]
enum Behaviors<DB: Database> {
  TimedMessage(TimedMessage),
}
