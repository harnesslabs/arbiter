use std::{
  fmt::{Display, Formatter},
  sync::atomic::{AtomicU64, Ordering},
  time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

static NEXT_MESSAGE_ID: AtomicU64 = AtomicU64::new(1);
static NEXT_CORRELATION_ID: AtomicU64 = AtomicU64::new(1);

fn now_unix_ms() -> u64 {
  match SystemTime::now().duration_since(UNIX_EPOCH) {
    Ok(duration) => duration.as_millis().try_into().unwrap_or(u64::MAX),
    Err(_) => 0,
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MessageKind(String);

impl MessageKind {
  pub fn new(value: impl Into<String>) -> Self {
    Self(value.into())
  }

  pub fn for_type<T>() -> Self {
    Self::new(std::any::type_name::<T>())
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

impl Display for MessageKind {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    f.write_str(self.as_str())
  }
}

impl From<&str> for MessageKind {
  fn from(value: &str) -> Self {
    Self::new(value)
  }
}

impl From<String> for MessageKind {
  fn from(value: String) -> Self {
    Self::new(value)
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SchemaVersion(pub u16);

impl Default for SchemaVersion {
  fn default() -> Self {
    Self(1)
  }
}

impl Display for SchemaVersion {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.0)
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MessageId(pub u64);

impl MessageId {
  pub fn next() -> Self {
    Self(NEXT_MESSAGE_ID.fetch_add(1, Ordering::Relaxed))
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CorrelationId(pub u64);

impl CorrelationId {
  pub fn next() -> Self {
    Self(NEXT_CORRELATION_ID.fetch_add(1, Ordering::Relaxed))
  }
}

impl From<MessageId> for CorrelationId {
  fn from(value: MessageId) -> Self {
    Self(value.0)
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(String);

impl NodeId {
  pub fn new(value: impl Into<String>) -> Self {
    Self(value.into())
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

impl Display for NodeId {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    f.write_str(self.as_str())
  }
}

impl From<String> for NodeId {
  fn from(value: String) -> Self {
    Self::new(value)
  }
}

impl From<&str> for NodeId {
  fn from(value: &str) -> Self {
    Self::new(value)
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentId(String);

impl AgentId {
  pub fn new(value: impl Into<String>) -> Self {
    Self(value.into())
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

impl Display for AgentId {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    f.write_str(self.as_str())
  }
}

impl From<String> for AgentId {
  fn from(value: String) -> Self {
    Self::new(value)
  }
}

impl From<&str> for AgentId {
  fn from(value: &str) -> Self {
    Self::new(value)
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Recipient {
  Broadcast,
  Agent(AgentId),
  Group(String),
}

impl Default for Recipient {
  fn default() -> Self {
    Self::Broadcast
  }
}

impl Recipient {
  pub fn matches_agent(&self, agent_id: &AgentId) -> bool {
    match self {
      Self::Broadcast => true,
      Self::Agent(target) => target == agent_id,
      Self::Group(_) => false,
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvelopeMeta {
  pub message_id: MessageId,
  pub correlation_id: Option<CorrelationId>,
  pub message_kind: MessageKind,
  pub schema_version: SchemaVersion,
  pub sender: Option<AgentId>,
  pub recipient: Recipient,
  pub created_at_unix_ms: u64,
}

impl EnvelopeMeta {
  pub fn new(message_kind: impl Into<MessageKind>) -> Self {
    Self {
      message_id: MessageId::next(),
      correlation_id: None,
      message_kind: message_kind.into(),
      schema_version: SchemaVersion::default(),
      sender: None,
      recipient: Recipient::Broadcast,
      created_at_unix_ms: now_unix_ms(),
    }
  }

  pub fn for_type<T>() -> Self {
    Self::new(MessageKind::for_type::<T>())
  }

  pub fn with_schema_version(mut self, schema_version: SchemaVersion) -> Self {
    self.schema_version = schema_version;
    self
  }

  pub fn with_sender(mut self, sender: impl Into<AgentId>) -> Self {
    self.sender = Some(sender.into());
    self
  }

  pub fn with_recipient(mut self, recipient: Recipient) -> Self {
    self.recipient = recipient;
    self
  }

  pub fn to_agent(mut self, agent_id: impl Into<AgentId>) -> Self {
    self.recipient = Recipient::Agent(agent_id.into());
    self
  }

  pub fn to_group(mut self, group: impl Into<String>) -> Self {
    self.recipient = Recipient::Group(group.into());
    self
  }

  pub fn broadcast(mut self) -> Self {
    self.recipient = Recipient::Broadcast;
    self
  }

  pub fn with_correlation_id(mut self, correlation_id: CorrelationId) -> Self {
    self.correlation_id = Some(correlation_id);
    self
  }
}
