use std::net::SocketAddr;

use thiserror::Error;

use crate::{
  network::tcp::{FramedTcpStream, TcpTransportError},
  observe::{EnvelopeSummary, ObserveEventKind, Observer},
  protocol::{
    AdvertiseAck, AdvertiseAgents, AgentId, CodecKind, HandshakeHello, Heartbeat, NodeId,
    WireEnvelope, WireFrame,
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

/// Lightweight node-side client for broker connections in the LAN MVP.
#[derive(Debug)]
pub struct NodeClient {
  node_id: NodeId,
  selected_codec: CodecKind,
  observer: Option<Observer>,
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

    Ok(Self { node_id, selected_codec: ack.selected_codec, observer, transport })
  }

  pub fn node_id(&self) -> &NodeId {
    &self.node_id
  }

  pub const fn selected_codec(&self) -> CodecKind {
    self.selected_codec
  }

  pub async fn advertise_agents(
    &mut self,
    agents: impl Into<Vec<AgentId>>,
  ) -> Result<AdvertiseAck, NodeClientError> {
    let agents = agents.into();
    let frame = WireFrame::AdvertiseAgents(AdvertiseAgents::new(agents.clone()));
    self.observe_frame_sent(&frame);
    self.transport.send_frame(&frame).await?;

    let Some(frame) = self.transport.recv_frame().await? else {
      return Err(NodeClientError::ConnectionClosed);
    };
    self.observe_frame_received(&frame);

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

  pub async fn send_envelope(&mut self, envelope: WireEnvelope) -> Result<(), NodeClientError> {
    let frame = WireFrame::Envelope(envelope);
    self.observe_frame_sent(&frame);
    self.transport.send_frame(&frame).await?;
    Ok(())
  }

  pub async fn send_heartbeat(&mut self) -> Result<(), NodeClientError> {
    let frame = WireFrame::Heartbeat(Heartbeat::now());
    self.observe_frame_sent(&frame);
    self.transport.send_frame(&frame).await?;
    Ok(())
  }

  pub async fn recv_frame(&mut self) -> Result<Option<WireFrame>, NodeClientError> {
    let frame = self.transport.recv_frame().await?;
    if let Some(frame) = &frame {
      self.observe_frame_received(frame);
    }
    Ok(frame)
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
