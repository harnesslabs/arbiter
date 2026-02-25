use std::{
  collections::HashMap,
  net::SocketAddr,
  sync::Arc,
  time::{Duration, Instant},
};

use thiserror::Error;
use tokio::{
  sync::{Mutex, mpsc, oneshot},
  task::JoinHandle,
};

use crate::{
  network::tcp::{FramedTcpStream, ServerHandshakeConfig, TcpTransportError, bind},
  observe::{EnvelopeSummary, ObserveEventKind, Observer},
  protocol::{AdvertiseAck, AdvertiseAgents, AgentId, NodeId, Recipient, WireEnvelope, WireFrame},
};

#[derive(Debug, Error)]
pub enum BrokerError {
  #[error(transparent)]
  Transport(#[from] TcpTransportError),
  #[error("broker task join failed: {0}")]
  Join(#[from] tokio::task::JoinError),
}

#[derive(Debug, Clone)]
pub struct BrokerConfig {
  pub listen_addr: SocketAddr,
  pub handshake: ServerHandshakeConfig,
  pub outbound_queue_capacity: usize,
  pub heartbeat_timeout: Duration,
  pub heartbeat_check_period: Duration,
  pub observer: Option<Observer>,
}

impl BrokerConfig {
  pub fn new(listen_addr: SocketAddr, broker_node_id: impl Into<NodeId>) -> Self {
    Self {
      listen_addr,
      handshake: ServerHandshakeConfig::new(broker_node_id),
      outbound_queue_capacity: 64,
      heartbeat_timeout: Duration::from_secs(30),
      heartbeat_check_period: Duration::from_secs(5),
      observer: None,
    }
  }

  pub fn with_handshake(mut self, handshake: ServerHandshakeConfig) -> Self {
    self.handshake = handshake;
    self
  }

  pub fn with_outbound_queue_capacity(mut self, capacity: usize) -> Self {
    self.outbound_queue_capacity = capacity.max(1);
    self
  }

  pub fn with_heartbeat_timeout(mut self, timeout: Duration) -> Self {
    self.heartbeat_timeout = timeout;
    self
  }

  pub fn with_heartbeat_check_period(mut self, period: Duration) -> Self {
    self.heartbeat_check_period = period;
    self
  }

  pub fn with_observer(mut self, observer: Observer) -> Self {
    self.observer = Some(observer);
    self
  }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BrokerSnapshot {
  pub connected_nodes: usize,
  pub registered_agents: usize,
  pub backpressure_drops: u64,
  pub unknown_recipient_drops: u64,
  pub unsupported_group_drops: u64,
}

#[derive(Debug)]
pub struct BrokerHandle {
  local_addr: SocketAddr,
  state: Arc<Mutex<BrokerState>>,
  shutdown_tx: Option<oneshot::Sender<()>>,
  task: JoinHandle<()>,
}

impl BrokerHandle {
  pub const fn local_addr(&self) -> SocketAddr {
    self.local_addr
  }

  pub async fn snapshot(&self) -> BrokerSnapshot {
    self.state.lock().await.snapshot()
  }

  pub async fn shutdown(mut self) -> Result<(), BrokerError> {
    if let Some(shutdown_tx) = self.shutdown_tx.take() {
      let _ = shutdown_tx.send(());
    }
    self.task.await?;
    Ok(())
  }
}

pub async fn spawn(config: BrokerConfig) -> Result<BrokerHandle, BrokerError> {
  let listener = bind(config.listen_addr).await?;
  let local_addr = listener
    .local_addr()
    .map_err(|source| BrokerError::Transport(TcpTransportError::Io { source }))?;
  let state = Arc::new(Mutex::new(BrokerState::default()));
  let (shutdown_tx, mut shutdown_rx) = oneshot::channel();
  let state_for_task = Arc::clone(&state);
  let config_for_task = config.clone();

  let task = tokio::spawn(async move {
    loop {
      tokio::select! {
        _ = &mut shutdown_rx => {
          break;
        }
        accept_result = listener.accept() => {
          let Ok((stream, _peer_addr)) = accept_result else {
            break;
          };

          let state = Arc::clone(&state_for_task);
          let connection_config = config_for_task.clone();
          tokio::spawn(async move {
            let mut transport = FramedTcpStream::new(stream);
            if let Err(error) = run_connection(&mut transport, state, connection_config).await {
              eprintln!("broker connection task error: {error}");
            }
          });
        }
      }
    }
  });

  Ok(BrokerHandle { local_addr, state, shutdown_tx: Some(shutdown_tx), task })
}

#[derive(Debug)]
struct PeerState {
  outbound_tx: mpsc::Sender<WireFrame>,
  last_seen: Instant,
}

#[derive(Debug, Default)]
struct RouteOutcome {
  recipients: Vec<NodeId>,
  backpressure_drops: u64,
  drop_reason: Option<String>,
}

#[derive(Debug, Default)]
struct BrokerState {
  peers: HashMap<NodeId, PeerState>,
  agent_routes: HashMap<AgentId, NodeId>,
  backpressure_drops: u64,
  unknown_recipient_drops: u64,
  unsupported_group_drops: u64,
}

impl BrokerState {
  fn snapshot(&self) -> BrokerSnapshot {
    BrokerSnapshot {
      connected_nodes: self.peers.len(),
      registered_agents: self.agent_routes.len(),
      backpressure_drops: self.backpressure_drops,
      unknown_recipient_drops: self.unknown_recipient_drops,
      unsupported_group_drops: self.unsupported_group_drops,
    }
  }

  fn register_peer(&mut self, node_id: NodeId, outbound_tx: mpsc::Sender<WireFrame>) {
    self.peers.insert(node_id, PeerState { outbound_tx, last_seen: Instant::now() });
  }

  fn touch_peer(&mut self, node_id: &NodeId) {
    if let Some(peer) = self.peers.get_mut(node_id) {
      peer.last_seen = Instant::now();
    }
  }

  fn register_agents(&mut self, node_id: &NodeId, advertise: &AdvertiseAgents) -> AdvertiseAck {
    for agent in &advertise.agents {
      self.agent_routes.insert(agent.clone(), node_id.clone());
    }
    AdvertiseAck { registered_agents: advertise.agents.len() }
  }

  fn route_envelope(&mut self, source_node_id: &NodeId, envelope: WireEnvelope) -> RouteOutcome {
    let mut outcome = RouteOutcome::default();
    match &envelope.meta.recipient {
      Recipient::Broadcast => {
        let frame = WireFrame::Envelope(envelope);
        for (target_node_id, peer) in &self.peers {
          if peer.outbound_tx.try_send(frame.clone()).is_err() {
            self.backpressure_drops += 1;
            outcome.backpressure_drops += 1;
          } else {
            outcome.recipients.push(target_node_id.clone());
          }
        }
        if outcome.recipients.is_empty() && outcome.backpressure_drops > 0 {
          outcome.drop_reason = Some("backpressure".to_string());
        }
      },
      Recipient::Agent(agent_id) => {
        let Some(target_node_id) = self.agent_routes.get(agent_id).cloned() else {
          self.unknown_recipient_drops += 1;
          outcome.drop_reason = Some("unknown_recipient".to_string());
          return outcome;
        };

        if let Some(peer) = self.peers.get(&target_node_id) {
          if peer.outbound_tx.try_send(WireFrame::Envelope(envelope)).is_err() {
            self.backpressure_drops += 1;
            outcome.backpressure_drops += 1;
            outcome.drop_reason = Some("backpressure".to_string());
          } else {
            outcome.recipients.push(target_node_id);
          }
        } else {
          self.unknown_recipient_drops += 1;
          outcome.drop_reason = Some("unknown_recipient".to_string());
        }
      },
      Recipient::Group(_) => {
        let _ = source_node_id;
        self.unsupported_group_drops += 1;
        outcome.drop_reason = Some("unsupported_group".to_string());
      },
    }
    outcome
  }

  fn remove_peer(&mut self, node_id: &NodeId) {
    self.peers.remove(node_id);
    self.agent_routes.retain(|_, owner| owner != node_id);
  }

  fn is_peer_timed_out(&self, node_id: &NodeId, timeout: Duration) -> bool {
    self.peers.get(node_id).is_some_and(|peer| peer.last_seen.elapsed() > timeout)
  }
}

async fn run_connection(
  transport: &mut FramedTcpStream,
  state: Arc<Mutex<BrokerState>>,
  config: BrokerConfig,
) -> Result<(), BrokerError> {
  let observer = config.observer.clone();
  let hello = match transport.server_handshake(&config.handshake).await {
    Ok(hello) => {
      if let Some(observer) = &observer
        && let Some(codec) =
          select_codec(&config.handshake.supported_codecs, &hello.supported_codecs)
      {
        observer.emit(ObserveEventKind::BrokerHandshakeAccepted {
          node_id: hello.node_id.clone(),
          protocol_version: hello.protocol_version,
          codec,
        });
      }
      hello
    },
    Err(error) => {
      if let Some(observer) = &observer {
        observer.emit(ObserveEventKind::BrokerHandshakeRejected { reason: error.to_string() });
      }
      return Err(error.into());
    },
  };
  let node_id = hello.node_id.clone();
  let (outbound_tx, mut outbound_rx) = mpsc::channel(config.outbound_queue_capacity.max(1));

  {
    let mut state = state.lock().await;
    state.register_peer(node_id.clone(), outbound_tx.clone());
  }
  if let Some(observer) = &observer {
    observer.emit(ObserveEventKind::BrokerNodeConnected { node_id: node_id.clone() });
  }

  let mut heartbeat_interval = tokio::time::interval(config.heartbeat_check_period);

  let disconnect_reason = loop {
    tokio::select! {
      _ = heartbeat_interval.tick() => {
        let timed_out = {
          let state = state.lock().await;
          state.is_peer_timed_out(&node_id, config.heartbeat_timeout)
        };
        if timed_out {
          if let Some(observer) = &observer {
            observer.emit(ObserveEventKind::BrokerHeartbeatTimeout { node_id: node_id.clone() });
          }
          break "heartbeat_timeout".to_string();
        }
      }
      outbound = outbound_rx.recv() => {
        match outbound {
          Some(frame) => transport.send_frame(&frame).await?,
          None => {
            break "outbound_queue_closed".to_string();
          },
        }
      }
      inbound = transport.recv_frame() => {
        let Some(frame) = inbound? else {
          break "peer_closed".to_string();
        };

        let mut state = state.lock().await;
        state.touch_peer(&node_id);

        match frame {
          WireFrame::Heartbeat(_) => {}
          WireFrame::AdvertiseAgents(advertise) => {
            let ack = state.register_agents(&node_id, &advertise);
            let agent_count = advertise.agents.len();
            drop(state);
            transport.send_frame(&WireFrame::AdvertiseAck(ack)).await?;
            if let Some(observer) = &observer {
              observer.emit(ObserveEventKind::BrokerAgentsAdvertised {
                node_id: node_id.clone(),
                agent_count,
              });
            }
          }
          WireFrame::Envelope(envelope) => {
            let summary = EnvelopeSummary::from_wire_envelope(&envelope);
            let outcome = state.route_envelope(&node_id, envelope);
            if let Some(observer) = &observer {
              if !outcome.recipients.is_empty() {
                observer.emit(ObserveEventKind::BrokerEnvelopeRouted {
                  source_node_id: node_id.clone(),
                  envelope: summary.clone(),
                  recipients: outcome.recipients.clone(),
                });
              }
              if let Some(reason) = outcome.drop_reason.clone() {
                observer.emit(ObserveEventKind::BrokerEnvelopeDropped {
                  source_node_id: node_id.clone(),
                  envelope: summary.clone(),
                  reason,
                });
              } else if outcome.backpressure_drops > 0 {
                observer.emit(ObserveEventKind::BrokerEnvelopeDropped {
                  source_node_id: node_id.clone(),
                  envelope: summary.clone(),
                  reason: "backpressure".to_string(),
                });
              }
            }
          }
          WireFrame::Hello(_)
          | WireFrame::HelloAck(_)
          | WireFrame::HelloReject(_)
          | WireFrame::AdvertiseAck(_) => {}
        }
      }
    }
  };

  let mut state = state.lock().await;
  state.remove_peer(&node_id);
  drop(state);
  if let Some(observer) = &observer {
    observer.emit(ObserveEventKind::BrokerNodeDisconnected { node_id, reason: disconnect_reason });
  }
  Ok(())
}

fn select_codec(
  server_supported: &[crate::protocol::CodecKind],
  client_supported: &[crate::protocol::CodecKind],
) -> Option<crate::protocol::CodecKind> {
  server_supported.iter().copied().find(|codec| client_supported.contains(codec))
}

#[cfg(test)]
mod tests {
  use std::sync::Arc;

  use tokio::time::{Duration, timeout};

  use super::*;
  use crate::{
    observe::{InMemoryRecorder, ObserveEventKind, Observer},
    protocol::{AgentId, CorrelationId, EnvelopeMeta, HandshakeHello, WireEnvelope},
    runtime::node::NodeClient,
  };

  async fn spawn_test_broker() -> BrokerHandle {
    spawn(
      BrokerConfig::new(SocketAddr::from(([127, 0, 0, 1], 0)), "broker-1")
        .with_heartbeat_timeout(Duration::from_millis(250))
        .with_heartbeat_check_period(Duration::from_millis(25))
        .with_outbound_queue_capacity(8),
    )
    .await
    .expect("spawn broker")
  }

  async fn connect_node(broker: &BrokerHandle, node_id: &str) -> NodeClient {
    NodeClient::connect(broker.local_addr(), HandshakeHello::new(node_id))
      .await
      .expect("connect node client")
  }

  #[tokio::test]
  async fn broker_routes_addressed_messages_to_registered_agent() {
    let broker = spawn_test_broker().await;

    let mut node_a = connect_node(&broker, "node-a").await;
    let mut node_b = connect_node(&broker, "node-b").await;

    node_b.advertise_agents(vec![AgentId::from("agent-b")]).await.expect("advertise agents");

    let envelope =
      WireEnvelope::new(EnvelopeMeta::new("example.msg").to_agent("agent-b"), vec![1, 2, 3]);
    node_a.send_envelope(envelope.clone()).await.expect("send addressed");

    let received = timeout(Duration::from_secs(1), node_b.recv_frame())
      .await
      .expect("node b recv timeout")
      .expect("node b recv result")
      .expect("node b frame");

    match received {
      WireFrame::Envelope(actual) => assert_eq!(actual, envelope),
      other => panic!("unexpected frame: {other:?}"),
    }

    let no_echo = timeout(Duration::from_millis(100), node_a.recv_frame()).await;
    assert!(no_echo.is_err(), "addressed message should not echo to sender node");

    drop(node_a);
    drop(node_b);
    broker.shutdown().await.expect("shutdown broker");
  }

  #[tokio::test]
  async fn broker_routes_broadcast_messages_to_connected_nodes() {
    let broker = spawn_test_broker().await;

    let mut node_a = connect_node(&broker, "node-a").await;
    let mut node_b = connect_node(&broker, "node-b").await;

    let envelope = WireEnvelope::new(EnvelopeMeta::new("example.broadcast").broadcast(), vec![9]);
    node_a.send_envelope(envelope.clone()).await.expect("send broadcast");

    let a_frame = timeout(Duration::from_secs(1), node_a.recv_frame())
      .await
      .expect("node a recv timeout")
      .expect("node a recv result")
      .expect("node a frame");
    let b_frame = timeout(Duration::from_secs(1), node_b.recv_frame())
      .await
      .expect("node b recv timeout")
      .expect("node b recv result")
      .expect("node b frame");

    for frame in [a_frame, b_frame] {
      match frame {
        WireFrame::Envelope(actual) => assert_eq!(actual, envelope),
        other => panic!("unexpected frame: {other:?}"),
      }
    }

    drop(node_a);
    drop(node_b);
    broker.shutdown().await.expect("shutdown broker");
  }

  #[tokio::test]
  async fn broker_removes_routes_when_node_disconnects() {
    let broker = spawn_test_broker().await;
    let mut node_b = connect_node(&broker, "node-b").await;

    node_b
      .advertise_agents(vec![AgentId::from("agent-b"), AgentId::from("agent-b-2")])
      .await
      .expect("advertise agents");

    let before = broker.snapshot().await;
    assert_eq!(before.connected_nodes, 1);
    assert_eq!(before.registered_agents, 2);

    drop(node_b);

    timeout(Duration::from_secs(2), async {
      loop {
        let snapshot = broker.snapshot().await;
        if snapshot.connected_nodes == 0 && snapshot.registered_agents == 0 {
          break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
      }
    })
    .await
    .expect("disconnect cleanup timeout");

    broker.shutdown().await.expect("shutdown broker");
  }

  #[tokio::test]
  async fn broker_disconnects_idle_nodes_after_heartbeat_timeout() {
    let broker = spawn_test_broker().await;
    let _node = connect_node(&broker, "idle-node").await;

    timeout(Duration::from_secs(2), async {
      loop {
        if broker.snapshot().await.connected_nodes == 0 {
          break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
      }
    })
    .await
    .expect("heartbeat timeout disconnect");

    broker.shutdown().await.expect("shutdown broker");
  }

  #[test]
  fn broker_counts_backpressure_drops_for_full_outbound_queue() {
    let (tx, _rx) = mpsc::channel(1);
    let mut state = BrokerState::default();
    state.register_peer(NodeId::from("node-1"), tx);

    let envelope = WireEnvelope::new(EnvelopeMeta::new("example.broadcast").broadcast(), vec![1]);

    state.route_envelope(&NodeId::from("source"), envelope.clone());
    assert_eq!(state.backpressure_drops, 0);

    state.route_envelope(&NodeId::from("source"), envelope);
    assert_eq!(state.backpressure_drops, 1);
  }

  #[tokio::test]
  async fn broker_observability_events_include_correlation_metadata() {
    let recorder = Arc::new(InMemoryRecorder::new());
    let observer = Observer::from_sink(Arc::clone(&recorder));
    let broker = spawn(
      BrokerConfig::new(SocketAddr::from(([127, 0, 0, 1], 0)), "broker-1")
        .with_observer(observer.clone())
        .with_heartbeat_timeout(Duration::from_secs(2))
        .with_heartbeat_check_period(Duration::from_millis(50)),
    )
    .await
    .expect("spawn broker");

    let mut node_a = NodeClient::connect_with_observer(
      broker.local_addr(),
      HandshakeHello::new("node-a"),
      Some(observer.clone()),
    )
    .await
    .expect("connect node a");
    let mut node_b = NodeClient::connect_with_observer(
      broker.local_addr(),
      HandshakeHello::new("node-b"),
      Some(observer.clone()),
    )
    .await
    .expect("connect node b");

    node_b.advertise_agents(vec![AgentId::from("agent-b")]).await.expect("advertise agents");

    let correlation_id = CorrelationId::next();
    let envelope = WireEnvelope::new(
      EnvelopeMeta::new("example.msg").with_correlation_id(correlation_id).to_agent("agent-b"),
      vec![7, 8, 9],
    );
    node_a.send_envelope(envelope).await.expect("send addressed");

    let _ = timeout(Duration::from_secs(1), node_b.recv_frame())
      .await
      .expect("node b recv timeout")
      .expect("node b recv result")
      .expect("node b frame");

    let events = recorder.snapshot();
    let routed = events.into_iter().find_map(|event| match event.kind {
      ObserveEventKind::BrokerEnvelopeRouted { envelope, .. } => Some(envelope),
      _ => None,
    });

    let routed = routed.expect("broker routed event");
    assert_eq!(routed.correlation_id, Some(correlation_id));

    drop(node_a);
    drop(node_b);
    broker.shutdown().await.expect("shutdown broker");
  }
}
