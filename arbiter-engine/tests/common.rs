use std::sync::Arc;

use anyhow::Result;
use arbiter_core::middleware::ArbiterMiddleware;
use arbiter_engine::{
    machine::{Behavior, ControlFlow, EventStream},
    messager::{Message, Messager, To},
};
use serde::{Deserialize, Serialize};

#[allow(unused)]
fn trace() {
    std::env::set_var("RUST_LOG", "trace");
    tracing_subscriber::fmt::init();
}

fn default_max_count() -> Option<u64> {
    Some(3)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TimedMessage {
    delay: u64,
    receive_data: String,
    send_data: String,
    #[serde(skip)]
    messager: Option<Messager>,
    #[serde(default)]
    count: u64,
    #[serde(default = "default_max_count")]
    max_count: Option<u64>,
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
        Self {
            delay,
            receive_data,
            send_data,
            messager: None,
            count: 0,
            max_count,
            startup_message,
        }
    }
}

#[async_trait::async_trait]
impl Behavior<Message> for TimedMessage {
    async fn startup(
        &mut self,
        _client: Arc<ArbiterMiddleware>,
        messager: Messager,
    ) -> Result<Option<EventStream<Message>>> {
        if let Some(startup_message) = &self.startup_message {
            messager.send(To::All, startup_message).await?;
        }
        self.messager = Some(messager.clone());
        Ok(Some(messager.stream()?))
    }

    async fn process(&mut self, event: Message) -> Result<ControlFlow> {
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
