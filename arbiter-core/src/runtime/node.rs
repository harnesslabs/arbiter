use std::net::SocketAddr;

use thiserror::Error;

use crate::{
  network::tcp::{FramedTcpStream, TcpTransportError},
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
  transport: FramedTcpStream,
}

impl NodeClient {
  pub async fn connect(addr: SocketAddr, hello: HandshakeHello) -> Result<Self, NodeClientError> {
    let node_id = hello.node_id.clone();
    let mut transport = FramedTcpStream::connect(addr).await?;
    let ack = transport.client_handshake(hello).await?;

    Ok(Self { node_id, selected_codec: ack.selected_codec, transport })
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
    self.transport.send_frame(&WireFrame::AdvertiseAgents(AdvertiseAgents::new(agents))).await?;

    let Some(frame) = self.transport.recv_frame().await? else {
      return Err(NodeClientError::ConnectionClosed);
    };

    match frame {
      WireFrame::AdvertiseAck(ack) => Ok(ack),
      other => Err(NodeClientError::UnexpectedFrame {
        expected: "advertise_ack",
        received: other.kind_name(),
      }),
    }
  }

  pub async fn send_envelope(&mut self, envelope: WireEnvelope) -> Result<(), NodeClientError> {
    self.transport.send_frame(&WireFrame::Envelope(envelope)).await?;
    Ok(())
  }

  pub async fn send_heartbeat(&mut self) -> Result<(), NodeClientError> {
    self.transport.send_frame(&WireFrame::Heartbeat(Heartbeat::now())).await?;
    Ok(())
  }

  pub async fn recv_frame(&mut self) -> Result<Option<WireFrame>, NodeClientError> {
    Ok(self.transport.recv_frame().await?)
  }
}
