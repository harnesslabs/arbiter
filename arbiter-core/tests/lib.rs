use arbiter_core::{
  environment::{Database, Middleware},
  error::ArbiterCoreError,
  machine::{Behavior, ControlFlow, CreateEngine, Engine, EngineType, EventStream},
  messager::{Message, Messager, To},
};
use arbiter_macros::Behaviors;
use serde::{Deserialize, Serialize};

mod engine;

#[derive(Debug, Deserialize, Serialize)]
struct MockBehavior;

#[async_trait::async_trait]
impl Behavior<()> for MockBehavior {
  async fn startup<DB>(
    &mut self,
    _middleware: Middleware<DB>,
    _messager: Messager,
  ) -> Result<Option<EventStream<()>>, ArbiterCoreError>
  where
    DB: Database + 'static,
    DB::Location: Send + Sync + 'static,
    DB::State: Send + Sync + 'static,
  {
    Ok(None)
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

#[async_trait::async_trait]
impl Behavior<Message> for TimedMessage {
  async fn startup<DB: Database>(
    &mut self,
    _middleware: Middleware<DB>,
    messager: Messager,
  ) -> Result<Option<EventStream<Message>>, ArbiterCoreError>
  where
    DB: Database + 'static,
    DB::Location: Send + Sync + 'static,
    DB::State: Send + Sync + 'static,
  {
    if let Some(startup_message) = &self.startup_message {
      messager.send(To::All, startup_message).await?;
    }
    self.messager = Some(messager.clone());
    Ok(Some(messager.stream()?))
  }

  async fn process(&mut self, event: Message) -> Result<ControlFlow, ArbiterCoreError> {
    if event.data == serde_json::to_string(&self.receive_data).unwrap() {
      let messager = self.messager.clone().unwrap();
      messager.send(To::All, self.send_data.clone()).await?;
      self.count += 1;
    }
    if self.count == self.max_count.unwrap_or(u64::MAX) {
      return Ok(ControlFlow::Halt);
    }
    Ok(ControlFlow::Continue)
  }
}

#[derive(Serialize, Deserialize, Debug, Behaviors)]
enum Behaviors {
  TimedMessage(TimedMessage),
}
