use std::{
  fmt::{Display, Formatter},
  sync::atomic::{AtomicU64, Ordering},
  time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize, de::DeserializeOwned};

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

/// Transport-level protocol version used during LAN handshakes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProtocolVersion {
  pub major: u16,
  pub minor: u16,
}

impl ProtocolVersion {
  pub const fn new(major: u16, minor: u16) -> Self {
    Self { major, minor }
  }

  pub const fn current() -> Self {
    Self::new(0, 1)
  }

  pub const fn matches(self, other: Self) -> bool {
    self.major == other.major && self.minor == other.minor
  }
}

impl Default for ProtocolVersion {
  fn default() -> Self {
    Self::current()
  }
}

impl Display for ProtocolVersion {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}.{}", self.major, self.minor)
  }
}

/// Built-in payload codec identifiers supported during handshake negotiation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CodecKind {
  Json,
}

impl Display for CodecKind {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Json => f.write_str("json"),
    }
  }
}

/// Generic codec interface for future LAN transports. JSON is the first implementation.
pub trait Codec {
  fn kind(&self) -> CodecKind;

  fn encode<T: Serialize>(&self, value: &T) -> Result<Vec<u8>, serde_json::Error>;

  fn decode<T: DeserializeOwned>(&self, bytes: &[u8]) -> Result<T, serde_json::Error>;
}

/// Default JSON codec implementation used in early LAN milestones.
#[derive(Debug, Clone, Copy, Default)]
pub struct JsonCodec;

impl Codec for JsonCodec {
  fn kind(&self) -> CodecKind {
    CodecKind::Json
  }

  fn encode<T: Serialize>(&self, value: &T) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec(value)
  }

  fn decode<T: DeserializeOwned>(&self, bytes: &[u8]) -> Result<T, serde_json::Error> {
    serde_json::from_slice(bytes)
  }
}

/// Serialized message frame for TCP/LAN transport.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireEnvelope {
  pub meta: EnvelopeMeta,
  pub payload: Vec<u8>,
}

impl WireEnvelope {
  pub fn new(meta: EnvelopeMeta, payload: Vec<u8>) -> Self {
    Self { meta, payload }
  }
}

/// Client->server handshake request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HandshakeHello {
  pub protocol_version: ProtocolVersion,
  pub node_id: NodeId,
  pub supported_codecs: Vec<CodecKind>,
  pub capabilities: Vec<String>,
  pub instance_name: Option<String>,
}

impl HandshakeHello {
  pub fn new(node_id: impl Into<NodeId>) -> Self {
    Self {
      protocol_version: ProtocolVersion::current(),
      node_id: node_id.into(),
      supported_codecs: vec![CodecKind::Json],
      capabilities: vec![],
      instance_name: None,
    }
  }

  pub fn with_protocol_version(mut self, version: ProtocolVersion) -> Self {
    self.protocol_version = version;
    self
  }

  pub fn with_supported_codecs(mut self, codecs: impl Into<Vec<CodecKind>>) -> Self {
    self.supported_codecs = codecs.into();
    self
  }

  pub fn with_capabilities(mut self, capabilities: impl Into<Vec<String>>) -> Self {
    self.capabilities = capabilities.into();
    self
  }

  pub fn with_instance_name(mut self, instance_name: impl Into<String>) -> Self {
    self.instance_name = Some(instance_name.into());
    self
  }
}

/// Server->client handshake success response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HandshakeAck {
  pub protocol_version: ProtocolVersion,
  pub broker_node_id: NodeId,
  pub selected_codec: CodecKind,
  pub capabilities: Vec<String>,
}

/// Handshake rejection reason for explicit protocol negotiation failures.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HandshakeRejectReason {
  UnsupportedProtocolVersion { expected: ProtocolVersion, received: ProtocolVersion },
  NoSharedCodec { server_supported: Vec<CodecKind>, client_supported: Vec<CodecKind> },
}

/// Server->client handshake rejection response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HandshakeReject {
  pub broker_node_id: Option<NodeId>,
  pub reason: HandshakeRejectReason,
}

/// Heartbeat frame used by broker/node connections.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Heartbeat {
  pub sent_at_unix_ms: u64,
}

impl Heartbeat {
  pub fn now() -> Self {
    Self { sent_at_unix_ms: now_unix_ms() }
  }
}

/// Node->broker registration of locally hosted agents for addressed routing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdvertiseAgents {
  pub agents: Vec<AgentId>,
}

impl AdvertiseAgents {
  pub fn new(agents: impl Into<Vec<AgentId>>) -> Self {
    Self { agents: agents.into() }
  }
}

/// Broker->node acknowledgement of agent registrations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdvertiseAck {
  pub registered_agents: usize,
}

/// Top-level framed payload exchanged over TCP.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WireFrame {
  Hello(HandshakeHello),
  HelloAck(HandshakeAck),
  HelloReject(HandshakeReject),
  AdvertiseAgents(AdvertiseAgents),
  AdvertiseAck(AdvertiseAck),
  Envelope(WireEnvelope),
  Heartbeat(Heartbeat),
}

impl WireFrame {
  pub const fn kind_name(&self) -> &'static str {
    match self {
      Self::Hello(_) => "hello",
      Self::HelloAck(_) => "hello_ack",
      Self::HelloReject(_) => "hello_reject",
      Self::AdvertiseAgents(_) => "advertise_agents",
      Self::AdvertiseAck(_) => "advertise_ack",
      Self::Envelope(_) => "envelope",
      Self::Heartbeat(_) => "heartbeat",
    }
  }
}
