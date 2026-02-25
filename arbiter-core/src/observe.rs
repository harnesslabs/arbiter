use std::{
  fs::{File, OpenOptions},
  io::Write,
  path::Path,
  sync::{Arc, Mutex},
  time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use crate::protocol::{
  CodecKind, CorrelationId, MessageId, MessageKind, NodeId, ProtocolVersion, Recipient,
  SchemaVersion, WireEnvelope,
};

fn now_unix_ms() -> u64 {
  match SystemTime::now().duration_since(UNIX_EPOCH) {
    Ok(duration) => duration.as_millis().try_into().unwrap_or(u64::MAX),
    Err(_) => 0,
  }
}

/// Wire-envelope metadata summary included in observability events and replay logs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvelopeSummary {
  pub message_id: MessageId,
  pub correlation_id: Option<CorrelationId>,
  pub message_kind: MessageKind,
  pub schema_version: SchemaVersion,
  pub sender: Option<crate::protocol::AgentId>,
  pub recipient: Recipient,
  pub payload_len_bytes: usize,
}

impl EnvelopeSummary {
  pub fn from_wire_envelope(envelope: &WireEnvelope) -> Self {
    Self {
      message_id: envelope.meta.message_id,
      correlation_id: envelope.meta.correlation_id,
      message_kind: envelope.meta.message_kind.clone(),
      schema_version: envelope.meta.schema_version,
      sender: envelope.meta.sender.clone(),
      recipient: envelope.meta.recipient.clone(),
      payload_len_bytes: envelope.payload.len(),
    }
  }
}

/// Structured observability events emitted by runtimes/transports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObserveEvent {
  pub timestamp_unix_ms: u64,
  pub kind: ObserveEventKind,
}

impl ObserveEvent {
  pub fn new(kind: ObserveEventKind) -> Self {
    Self { timestamp_unix_ms: now_unix_ms(), kind }
  }
}

/// Event payloads for broker/node/transport lifecycle and message routing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObserveEventKind {
  BrokerHandshakeAccepted {
    node_id: NodeId,
    protocol_version: ProtocolVersion,
    codec: CodecKind,
  },
  BrokerHandshakeRejected {
    reason: String,
  },
  BrokerNodeConnected {
    node_id: NodeId,
  },
  BrokerNodeDisconnected {
    node_id: NodeId,
    reason: String,
  },
  BrokerAgentsAdvertised {
    node_id: NodeId,
    agent_count: usize,
  },
  BrokerEnvelopeRouted {
    source_node_id: NodeId,
    envelope: EnvelopeSummary,
    recipients: Vec<NodeId>,
  },
  BrokerEnvelopeDropped {
    source_node_id: NodeId,
    envelope: EnvelopeSummary,
    reason: String,
  },
  BrokerHeartbeatTimeout {
    node_id: NodeId,
  },
  NodeConnected {
    node_id: NodeId,
    broker_node_id: NodeId,
    protocol_version: ProtocolVersion,
    codec: CodecKind,
  },
  NodeAgentsAdvertised {
    node_id: NodeId,
    agent_count: usize,
  },
  NodeFrameSent {
    node_id: NodeId,
    frame_kind: String,
    envelope: Option<EnvelopeSummary>,
  },
  NodeFrameReceived {
    node_id: NodeId,
    frame_kind: String,
    envelope: Option<EnvelopeSummary>,
  },
}

/// Sink trait for observability events.
pub trait EventSink: Send + Sync {
  fn emit(&self, event: &ObserveEvent);
}

#[derive(Default)]
struct ObserverInner {
  sinks: Mutex<Vec<Arc<dyn EventSink>>>,
}

/// Clonable event dispatcher that fan-outs events to registered sinks.
#[derive(Clone, Default)]
pub struct Observer {
  inner: Arc<ObserverInner>,
}

impl std::fmt::Debug for Observer {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let sink_count = self.inner.sinks.lock().map(|sinks| sinks.len()).unwrap_or(0);
    f.debug_struct("Observer").field("sink_count", &sink_count).finish()
  }
}

impl Observer {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn from_sink<S>(sink: S) -> Self
  where
    S: EventSink + 'static,
  {
    let observer = Self::new();
    observer.add_sink(sink);
    observer
  }

  pub fn add_sink<S>(&self, sink: S)
  where
    S: EventSink + 'static,
  {
    self.inner.sinks.lock().expect("observer sink lock poisoned").push(Arc::new(sink));
  }

  pub fn emit(&self, kind: ObserveEventKind) {
    self.emit_event(ObserveEvent::new(kind));
  }

  pub fn emit_event(&self, event: ObserveEvent) {
    let sinks = self.inner.sinks.lock().expect("observer sink lock poisoned").clone();
    for sink in sinks {
      sink.emit(&event);
    }
  }

  pub fn is_enabled(&self) -> bool {
    !self.inner.sinks.lock().expect("observer sink lock poisoned").is_empty()
  }
}

/// In-memory event recorder sink useful for tests and interactive inspection.
#[derive(Debug, Default)]
pub struct InMemoryRecorder {
  events: Mutex<Vec<ObserveEvent>>,
}

impl InMemoryRecorder {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn snapshot(&self) -> Vec<ObserveEvent> {
    self.events.lock().expect("recorder lock poisoned").clone()
  }
}

impl EventSink for Arc<InMemoryRecorder> {
  fn emit(&self, event: &ObserveEvent) {
    self.events.lock().expect("recorder lock poisoned").push(event.clone());
  }
}

/// JSON Lines sink for durable event recording and replay inputs.
#[derive(Debug)]
pub struct JsonlFileSink {
  file: Mutex<File>,
}

impl JsonlFileSink {
  pub fn create(path: impl AsRef<Path>) -> std::io::Result<Self> {
    let file = OpenOptions::new().create(true).append(true).open(path)?;
    Ok(Self { file: Mutex::new(file) })
  }
}

impl EventSink for JsonlFileSink {
  fn emit(&self, event: &ObserveEvent) {
    let Ok(mut file) = self.file.lock() else {
      return;
    };

    let Ok(line) = serde_json::to_vec(event) else {
      return;
    };

    let _ = file.write_all(&line);
    let _ = file.write_all(b"\n");
    let _ = file.flush();
  }
}

/// `tracing` sink for runtime-integrated structured logging.
#[derive(Debug, Default, Clone, Copy)]
pub struct TracingSink;

impl EventSink for TracingSink {
  fn emit(&self, event: &ObserveEvent) {
    tracing::info!(target: "arbiter::observe", event = ?event, "arbiter event");
  }
}

#[cfg(test)]
mod tests {
  use std::{fs, sync::Arc};

  use crate::protocol::{CorrelationId, EnvelopeMeta, WireEnvelope};

  use super::*;

  #[test]
  fn in_memory_recorder_captures_envelope_correlation_metadata() {
    let recorder = Arc::new(InMemoryRecorder::new());
    let observer = Observer::from_sink(Arc::clone(&recorder));

    let envelope = WireEnvelope::new(
      EnvelopeMeta::new("example.msg")
        .with_correlation_id(CorrelationId::next())
        .to_agent("agent-1"),
      vec![1, 2, 3, 4],
    );
    let summary = EnvelopeSummary::from_wire_envelope(&envelope);

    observer.emit(ObserveEventKind::BrokerEnvelopeDropped {
      source_node_id: "node-a".into(),
      envelope: summary.clone(),
      reason: "unknown recipient".to_string(),
    });

    let events = recorder.snapshot();
    assert_eq!(events.len(), 1);
    match &events[0].kind {
      ObserveEventKind::BrokerEnvelopeDropped { envelope, .. } => {
        assert_eq!(envelope.correlation_id, summary.correlation_id);
        assert_eq!(envelope.message_id, summary.message_id);
        assert_eq!(envelope.message_kind, summary.message_kind);
      },
      other => panic!("unexpected event: {other:?}"),
    }
  }

  #[test]
  fn jsonl_file_sink_writes_serializable_events() {
    let path = std::env::temp_dir().join(format!(
      "arbiter-observe-test-{}.jsonl",
      std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("unix time")
        .as_nanos()
    ));

    let sink = JsonlFileSink::create(&path).expect("create jsonl sink");
    let observer = Observer::from_sink(sink);

    observer.emit(ObserveEventKind::BrokerNodeConnected { node_id: "node-a".into() });

    let contents = fs::read_to_string(&path).expect("read jsonl output");
    let line = contents.lines().next().expect("jsonl line");
    let parsed: ObserveEvent = serde_json::from_str(line).expect("parse observe event");
    assert!(matches!(
      parsed.kind,
      ObserveEventKind::BrokerNodeConnected { ref node_id } if node_id.as_str() == "node-a"
    ));

    let _ = fs::remove_file(&path);
  }
}
