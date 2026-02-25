use std::net::SocketAddr;

use thiserror::Error;
use tokio::{
  io::{AsyncReadExt, AsyncWriteExt},
  net::{TcpListener, TcpStream},
};

use crate::protocol::{
  CodecKind, HandshakeAck, HandshakeHello, HandshakeReject, HandshakeRejectReason, NodeId,
  ProtocolVersion, WireFrame,
};

/// Default maximum frame size for length-delimited TCP transport.
pub const DEFAULT_MAX_FRAME_LEN: usize = 8 * 1024 * 1024;

/// Errors emitted by the framed TCP transport and handshake helpers.
#[derive(Debug, Error)]
pub enum TcpTransportError {
  #[error("io error: {source}")]
  Io {
    #[from]
    source: std::io::Error,
  },
  #[error("frame is too large: {len} bytes exceeds max {max} bytes")]
  FrameTooLarge { len: usize, max: usize },
  #[error("failed to encode frame as json: {source}")]
  EncodeFrameJson { source: serde_json::Error },
  #[error("failed to decode frame from json: {source}")]
  DecodeFrameJson { source: serde_json::Error },
  #[error("connection closed")]
  ConnectionClosed,
  #[error("unexpected frame during handshake: expected {expected}, received {received}")]
  UnexpectedFrame { expected: &'static str, received: &'static str },
  #[error("protocol version mismatch: expected {expected}, received {received}")]
  ProtocolVersionMismatch { expected: ProtocolVersion, received: ProtocolVersion },
  #[error("codec negotiation failed")]
  NoSharedCodec { server_supported: Vec<CodecKind>, client_supported: Vec<CodecKind> },
  #[error("handshake rejected: {reason:?}")]
  HandshakeRejected { reason: HandshakeRejectReason },
}

/// Server-side handshake configuration.
#[derive(Debug, Clone)]
pub struct ServerHandshakeConfig {
  pub broker_node_id: NodeId,
  pub protocol_version: ProtocolVersion,
  pub supported_codecs: Vec<CodecKind>,
  pub capabilities: Vec<String>,
}

impl ServerHandshakeConfig {
  pub fn new(broker_node_id: impl Into<NodeId>) -> Self {
    Self {
      broker_node_id: broker_node_id.into(),
      protocol_version: ProtocolVersion::current(),
      supported_codecs: vec![CodecKind::Json],
      capabilities: vec![],
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
}

/// Length-delimited TCP transport that exchanges JSON-encoded [`WireFrame`] values.
pub struct FramedTcpStream {
  stream: TcpStream,
  max_frame_len: usize,
}

impl std::fmt::Debug for FramedTcpStream {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("FramedTcpStream")
      .field("peer_addr", &self.stream.peer_addr().ok())
      .field("local_addr", &self.stream.local_addr().ok())
      .field("max_frame_len", &self.max_frame_len)
      .finish()
  }
}

impl FramedTcpStream {
  pub fn new(stream: TcpStream) -> Self {
    Self::with_max_frame_len(stream, DEFAULT_MAX_FRAME_LEN)
  }

  pub const fn with_max_frame_len(stream: TcpStream, max_frame_len: usize) -> Self {
    Self { stream, max_frame_len }
  }

  pub async fn connect(addr: SocketAddr) -> Result<Self, TcpTransportError> {
    let stream = TcpStream::connect(addr).await?;
    Ok(Self::new(stream))
  }

  pub const fn max_frame_len(&self) -> usize {
    self.max_frame_len
  }

  pub async fn send_frame(&mut self, frame: &WireFrame) -> Result<(), TcpTransportError> {
    let encoded =
      serde_json::to_vec(frame).map_err(|source| TcpTransportError::EncodeFrameJson { source })?;
    self.send_bytes(&encoded).await
  }

  pub async fn recv_frame(&mut self) -> Result<Option<WireFrame>, TcpTransportError> {
    let Some(bytes) = self.recv_bytes().await? else {
      return Ok(None);
    };

    serde_json::from_slice(&bytes)
      .map(Some)
      .map_err(|source| TcpTransportError::DecodeFrameJson { source })
  }

  pub async fn client_handshake(
    &mut self,
    hello: HandshakeHello,
  ) -> Result<HandshakeAck, TcpTransportError> {
    self.send_frame(&WireFrame::Hello(hello)).await?;

    let Some(frame) = self.recv_frame().await? else {
      return Err(TcpTransportError::ConnectionClosed);
    };

    match frame {
      WireFrame::HelloAck(ack) => Ok(ack),
      WireFrame::HelloReject(reject) => {
        Err(TcpTransportError::HandshakeRejected { reason: reject.reason })
      },
      other => Err(TcpTransportError::UnexpectedFrame {
        expected: "hello_ack|hello_reject",
        received: other.kind_name(),
      }),
    }
  }

  pub async fn server_handshake(
    &mut self,
    config: &ServerHandshakeConfig,
  ) -> Result<HandshakeHello, TcpTransportError> {
    let Some(frame) = self.recv_frame().await? else {
      return Err(TcpTransportError::ConnectionClosed);
    };

    let hello = match frame {
      WireFrame::Hello(hello) => hello,
      other => {
        return Err(TcpTransportError::UnexpectedFrame {
          expected: "hello",
          received: other.kind_name(),
        });
      },
    };

    if !hello.protocol_version.matches(config.protocol_version) {
      let reason = HandshakeRejectReason::UnsupportedProtocolVersion {
        expected: config.protocol_version,
        received: hello.protocol_version,
      };
      let _ = self
        .send_frame(&WireFrame::HelloReject(HandshakeReject {
          broker_node_id: Some(config.broker_node_id.clone()),
          reason: reason.clone(),
        }))
        .await;
      return Err(TcpTransportError::ProtocolVersionMismatch {
        expected: config.protocol_version,
        received: hello.protocol_version,
      });
    }

    let Some(selected_codec) = select_codec(&config.supported_codecs, &hello.supported_codecs)
    else {
      let reason = HandshakeRejectReason::NoSharedCodec {
        server_supported: config.supported_codecs.clone(),
        client_supported: hello.supported_codecs.clone(),
      };
      let _ = self
        .send_frame(&WireFrame::HelloReject(HandshakeReject {
          broker_node_id: Some(config.broker_node_id.clone()),
          reason: reason.clone(),
        }))
        .await;
      return Err(TcpTransportError::NoSharedCodec {
        server_supported: config.supported_codecs.clone(),
        client_supported: hello.supported_codecs.clone(),
      });
    };

    self
      .send_frame(&WireFrame::HelloAck(HandshakeAck {
        protocol_version: config.protocol_version,
        broker_node_id: config.broker_node_id.clone(),
        selected_codec,
        capabilities: config.capabilities.clone(),
      }))
      .await?;

    Ok(hello)
  }

  async fn send_bytes(&mut self, bytes: &[u8]) -> Result<(), TcpTransportError> {
    if bytes.len() > self.max_frame_len || bytes.len() > u32::MAX as usize {
      return Err(TcpTransportError::FrameTooLarge { len: bytes.len(), max: self.max_frame_len });
    }

    self.stream.write_u32(bytes.len() as u32).await?;
    self.stream.write_all(bytes).await?;
    self.stream.flush().await?;
    Ok(())
  }

  async fn recv_bytes(&mut self) -> Result<Option<Vec<u8>>, TcpTransportError> {
    let frame_len = match self.stream.read_u32().await {
      Ok(len) => len as usize,
      Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
      Err(source) => return Err(TcpTransportError::Io { source }),
    };

    if frame_len > self.max_frame_len {
      return Err(TcpTransportError::FrameTooLarge { len: frame_len, max: self.max_frame_len });
    }

    let mut bytes = vec![0; frame_len];
    self.stream.read_exact(&mut bytes).await?;
    Ok(Some(bytes))
  }
}

pub async fn bind(addr: SocketAddr) -> Result<TcpListener, TcpTransportError> {
  Ok(TcpListener::bind(addr).await?)
}

pub async fn accept(
  listener: &TcpListener,
) -> Result<(FramedTcpStream, SocketAddr), TcpTransportError> {
  let (stream, peer_addr) = listener.accept().await?;
  Ok((FramedTcpStream::new(stream), peer_addr))
}

fn select_codec(
  server_supported: &[CodecKind],
  client_supported: &[CodecKind],
) -> Option<CodecKind> {
  server_supported.iter().copied().find(|codec| client_supported.contains(codec))
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::protocol::{
    HandshakeHello, HandshakeRejectReason, Heartbeat, ProtocolVersion, WireFrame,
  };

  async fn localhost_listener() -> TcpListener {
    bind(SocketAddr::from(([127, 0, 0, 1], 0))).await.expect("bind localhost listener")
  }

  #[tokio::test]
  async fn framed_transport_round_trip() {
    let listener = localhost_listener().await;
    let addr = listener.local_addr().expect("listener addr");

    let server = tokio::spawn(async move {
      let (mut server_stream, _) = accept(&listener).await.expect("accept server");

      let frame = server_stream.recv_frame().await.expect("recv frame").expect("frame present");
      assert!(matches!(frame, WireFrame::Heartbeat(_)));

      server_stream
        .send_frame(&WireFrame::Heartbeat(Heartbeat::now()))
        .await
        .expect("send heartbeat");
    });

    let mut client = FramedTcpStream::connect(addr).await.expect("connect client");
    client.send_frame(&WireFrame::Heartbeat(Heartbeat::now())).await.expect("send heartbeat");

    let echoed = client.recv_frame().await.expect("recv echoed").expect("echoed frame");
    assert!(matches!(echoed, WireFrame::Heartbeat(_)));

    server.await.expect("server task");
  }

  #[tokio::test]
  async fn handshake_success() {
    let listener = localhost_listener().await;
    let addr = listener.local_addr().expect("listener addr");

    let server = tokio::spawn(async move {
      let (mut server_stream, _) = accept(&listener).await.expect("accept server");
      let config = ServerHandshakeConfig::new("broker-1")
        .with_protocol_version(ProtocolVersion::new(0, 1))
        .with_supported_codecs(vec![CodecKind::Json]);
      let hello = server_stream.server_handshake(&config).await.expect("server handshake");
      assert_eq!(hello.node_id.as_str(), "client-1");
    });

    let mut client = FramedTcpStream::connect(addr).await.expect("connect client");
    let ack =
      client.client_handshake(HandshakeHello::new("client-1")).await.expect("client handshake");

    assert_eq!(ack.broker_node_id.as_str(), "broker-1");
    assert_eq!(ack.selected_codec, CodecKind::Json);
    assert_eq!(ack.protocol_version, ProtocolVersion::new(0, 1));

    server.await.expect("server task");
  }

  #[tokio::test]
  async fn handshake_rejects_protocol_version_mismatch() {
    let listener = localhost_listener().await;
    let addr = listener.local_addr().expect("listener addr");

    let server = tokio::spawn(async move {
      let (mut server_stream, _) = accept(&listener).await.expect("accept server");
      let config =
        ServerHandshakeConfig::new("broker-1").with_protocol_version(ProtocolVersion::new(0, 1));
      let error = server_stream.server_handshake(&config).await.expect_err("expected mismatch");
      match error {
        TcpTransportError::ProtocolVersionMismatch { expected, received } => {
          assert_eq!(expected, ProtocolVersion::new(0, 1));
          assert_eq!(received, ProtocolVersion::new(9, 9));
        },
        other => panic!("unexpected error: {other:?}"),
      }
    });

    let mut client = FramedTcpStream::connect(addr).await.expect("connect client");
    let error = client
      .client_handshake(
        HandshakeHello::new("client-1").with_protocol_version(ProtocolVersion::new(9, 9)),
      )
      .await
      .expect_err("expected handshake rejection");

    match error {
      TcpTransportError::HandshakeRejected { reason } => match reason {
        HandshakeRejectReason::UnsupportedProtocolVersion { expected, received } => {
          assert_eq!(expected, ProtocolVersion::new(0, 1));
          assert_eq!(received, ProtocolVersion::new(9, 9));
        },
        other => panic!("unexpected reject reason: {other:?}"),
      },
      other => panic!("unexpected client error: {other:?}"),
    }

    server.await.expect("server task");
  }

  #[tokio::test]
  async fn handshake_rejects_when_no_shared_codec() {
    let listener = localhost_listener().await;
    let addr = listener.local_addr().expect("listener addr");

    let server = tokio::spawn(async move {
      let (mut server_stream, _) = accept(&listener).await.expect("accept server");
      let config =
        ServerHandshakeConfig::new("broker-1").with_supported_codecs(vec![CodecKind::Json]);
      let error =
        server_stream.server_handshake(&config).await.expect_err("expected codec mismatch");
      match error {
        TcpTransportError::NoSharedCodec { server_supported, client_supported } => {
          assert_eq!(server_supported, vec![CodecKind::Json]);
          assert!(client_supported.is_empty());
        },
        other => panic!("unexpected server error: {other:?}"),
      }
    });

    let mut client = FramedTcpStream::connect(addr).await.expect("connect client");
    let error = client
      .client_handshake(
        HandshakeHello::new("client-1").with_supported_codecs(Vec::<CodecKind>::new()),
      )
      .await
      .expect_err("expected handshake rejection");

    match error {
      TcpTransportError::HandshakeRejected { reason } => match reason {
        HandshakeRejectReason::NoSharedCodec { server_supported, client_supported } => {
          assert_eq!(server_supported, vec![CodecKind::Json]);
          assert!(client_supported.is_empty());
        },
        other => panic!("unexpected reject reason: {other:?}"),
      },
      other => panic!("unexpected client error: {other:?}"),
    }

    server.await.expect("server task");
  }
}
