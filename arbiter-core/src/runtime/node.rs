use std::{collections::VecDeque, net::SocketAddr, time::Duration};

use thiserror::Error;

use crate::{
  network::tcp::{FramedTcpStream, TcpTransportError},
  observe::{EnvelopeSummary, ObserveEventKind, Observer},
  protocol::{
    AdvertiseAck, AdvertiseAgents, AgentId, CodecKind, GroupAck, GroupJoin, GroupLeave,
    HandshakeHello, Heartbeat, LookupAgent, LookupAgentResult, NodeId, WireEnvelope, WireFrame,
  },
};

/// Errors returned by the broker-connected node client helper.
#[derive(Debug, Error)]
pub enum NodeClientError {
  #[error(transparent)]
  Transport(#[from] TcpTransportError),
  #[error("unexpected frame: expected {expected}, received {received}")]
  UnexpectedFrame { expected: &'static str, received: &'static str },
  #[error("connection closed")]
  ConnectionClosed,
}

#[derive(Debug, Error)]
pub enum NodeRequestError {
  #[error(transparent)]
  Client(#[from] NodeClientError),
  #[error("request timed out after {timeout:?}")]
  Timeout { timeout: Duration },
}

/// Lightweight node-side client for broker connections in the LAN MVP.
#[derive(Debug)]
pub struct NodeClient {
  node_id: NodeId,
  selected_codec: CodecKind,
  broker_capabilities: Vec<String>,
  observer: Option<Observer>,
  pending_frames: VecDeque<WireFrame>,
  transport: FramedTcpStream,
}

impl NodeClient {
  pub async fn connect(addr: SocketAddr, hello: HandshakeHello) -> Result<Self, NodeClientError> {
    Self::connect_with_observer(addr, hello, None).await
  }

  pub async fn connect_with_observer(
    addr: SocketAddr,
    hello: HandshakeHello,
    observer: Option<Observer>,
  ) -> Result<Self, NodeClientError> {
    let node_id = hello.node_id.clone();
    let mut transport = FramedTcpStream::connect(addr).await?;
    let ack = transport.client_handshake(hello).await?;

    if let Some(observer) = &observer {
      observer.emit(ObserveEventKind::NodeConnected {
        node_id: node_id.clone(),
        broker_node_id: ack.broker_node_id.clone(),
        protocol_version: ack.protocol_version,
        codec: ack.selected_codec,
      });
    }

    Ok(Self {
      node_id,
      selected_codec: ack.selected_codec,
      broker_capabilities: ack.capabilities.clone(),
      observer,
      pending_frames: VecDeque::new(),
      transport,
    })
  }

  pub fn node_id(&self) -> &NodeId {
    &self.node_id
  }

  pub const fn selected_codec(&self) -> CodecKind {
    self.selected_codec
  }

  pub fn broker_capabilities(&self) -> &[String] {
    &self.broker_capabilities
  }

  pub fn broker_supports_capability(&self, capability: &str) -> bool {
    self.broker_capabilities.iter().any(|candidate| candidate == capability)
  }

  pub async fn advertise_agents(
    &mut self,
    agents: impl Into<Vec<AgentId>>,
  ) -> Result<AdvertiseAck, NodeClientError> {
    let agents = agents.into();
    self.send_frame(WireFrame::AdvertiseAgents(AdvertiseAgents::new(agents.clone()))).await?;
    let frame = self.recv_required_frame().await?;

    match frame {
      WireFrame::AdvertiseAck(ack) => {
        if let Some(observer) = &self.observer {
          observer.emit(ObserveEventKind::NodeAgentsAdvertised {
            node_id: self.node_id.clone(),
            agent_count: agents.len(),
          });
        }
        Ok(ack)
      },
      other => Err(NodeClientError::UnexpectedFrame {
        expected: "advertise_ack",
        received: other.kind_name(),
      }),
    }
  }

  pub async fn join_groups(
    &mut self,
    groups: impl Into<Vec<String>>,
  ) -> Result<GroupAck, NodeClientError> {
    self.send_frame(WireFrame::GroupJoin(GroupJoin::new(groups))).await?;
    match self.recv_required_frame().await? {
      WireFrame::GroupAck(ack) => Ok(ack),
      other => {
        Err(NodeClientError::UnexpectedFrame { expected: "group_ack", received: other.kind_name() })
      },
    }
  }

  pub async fn leave_groups(
    &mut self,
    groups: impl Into<Vec<String>>,
  ) -> Result<GroupAck, NodeClientError> {
    self.send_frame(WireFrame::GroupLeave(GroupLeave::new(groups))).await?;
    match self.recv_required_frame().await? {
      WireFrame::GroupAck(ack) => Ok(ack),
      other => {
        Err(NodeClientError::UnexpectedFrame { expected: "group_ack", received: other.kind_name() })
      },
    }
  }

  pub async fn lookup_agent(
    &mut self,
    agent_id: impl Into<AgentId>,
  ) -> Result<LookupAgentResult, NodeClientError> {
    self.send_frame(WireFrame::LookupAgent(LookupAgent::new(agent_id))).await?;
    match self.recv_required_frame().await? {
      WireFrame::LookupAgentResult(result) => Ok(result),
      other => Err(NodeClientError::UnexpectedFrame {
        expected: "lookup_agent_result",
        received: other.kind_name(),
      }),
    }
  }

  pub async fn send_envelope(&mut self, envelope: WireEnvelope) -> Result<(), NodeClientError> {
    self.send_frame(WireFrame::Envelope(envelope)).await
  }

  pub async fn request_envelope_with_timeout(
    &mut self,
    request: WireEnvelope,
    timeout: Duration,
  ) -> Result<WireEnvelope, NodeRequestError> {
    let expected_correlation_id = crate::protocol::CorrelationId::from(request.meta.message_id);
    self.send_envelope(request).await?;

    // First, scan any buffered frames once without starving on non-matching entries.
    let pending_len = self.pending_frames.len();
    for _ in 0..pending_len {
      let frame = self.pending_frames.pop_front().expect("pending queue length drift");
      match frame {
        WireFrame::Envelope(envelope)
          if envelope.meta.correlation_id == Some(expected_correlation_id) =>
        {
          return Ok(envelope);
        },
        other => self.pending_frames.push_back(other),
      }
    }

    let deadline = tokio::time::Instant::now() + timeout;
    loop {
      let now = tokio::time::Instant::now();
      if now >= deadline {
        return Err(NodeRequestError::Timeout { timeout });
      }

      let remaining = deadline - now;
      let frame = tokio::time::timeout(remaining, self.recv_required_transport_frame())
        .await
        .map_err(|_| NodeRequestError::Timeout { timeout })??;

      match frame {
        WireFrame::Envelope(envelope)
          if envelope.meta.correlation_id == Some(expected_correlation_id) =>
        {
          return Ok(envelope);
        },
        other => {
          self.pending_frames.push_back(other);
        },
      }
    }
  }

  pub async fn send_heartbeat(&mut self) -> Result<(), NodeClientError> {
    self.send_frame(WireFrame::Heartbeat(Heartbeat::now())).await
  }

  pub async fn recv_frame(&mut self) -> Result<Option<WireFrame>, NodeClientError> {
    if let Some(frame) = self.pending_frames.pop_front() {
      return Ok(Some(frame));
    }

    let frame = self.recv_transport_frame().await?;
    Ok(frame)
  }

  async fn recv_transport_frame(&mut self) -> Result<Option<WireFrame>, NodeClientError> {
    let frame = self.transport.recv_frame().await?;
    if let Some(frame) = &frame {
      self.observe_frame_received(frame);
    }
    Ok(frame)
  }

  async fn send_frame(&mut self, frame: WireFrame) -> Result<(), NodeClientError> {
    self.observe_frame_sent(&frame);
    self.transport.send_frame(&frame).await?;
    Ok(())
  }

  async fn recv_required_frame(&mut self) -> Result<WireFrame, NodeClientError> {
    self.recv_frame().await?.ok_or(NodeClientError::ConnectionClosed)
  }

  async fn recv_required_transport_frame(&mut self) -> Result<WireFrame, NodeClientError> {
    self.recv_transport_frame().await?.ok_or(NodeClientError::ConnectionClosed)
  }

  fn observe_frame_sent(&self, frame: &WireFrame) {
    let Some(observer) = &self.observer else { return };
    observer.emit(ObserveEventKind::NodeFrameSent {
      node_id: self.node_id.clone(),
      frame_kind: frame.kind_name().to_string(),
      envelope: frame_envelope_summary(frame),
    });
  }

  fn observe_frame_received(&self, frame: &WireFrame) {
    let Some(observer) = &self.observer else { return };
    observer.emit(ObserveEventKind::NodeFrameReceived {
      node_id: self.node_id.clone(),
      frame_kind: frame.kind_name().to_string(),
      envelope: frame_envelope_summary(frame),
    });
  }
}

fn frame_envelope_summary(frame: &WireFrame) -> Option<EnvelopeSummary> {
  match frame {
    WireFrame::Envelope(envelope) => Some(EnvelopeSummary::from_wire_envelope(envelope)),
    _ => None,
  }
}
