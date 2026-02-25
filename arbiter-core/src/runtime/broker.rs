use std::{
  collections::{HashMap, HashSet},
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
  protocol::{
    AdvertiseAck, AdvertiseAgents, AgentId, GroupAck, GroupJoin, GroupLeave, LookupAgentResult,
    NodeId, Recipient, WireEnvelope, WireFrame,
  },
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

  pub fn with_coordination_capabilities(mut self) -> Self {
    self.handshake.capabilities = crate::protocol::capabilities::coordination_defaults();
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
            let transport = FramedTcpStream::new(stream);
            if let Err(error) = run_connection(transport, state, connection_config).await {
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
  groups: HashMap<String, HashSet<NodeId>>,
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

  fn join_groups(&mut self, node_id: &NodeId, join: &GroupJoin) -> GroupAck {
    for group in &join.groups {
      self.groups.entry(group.clone()).or_default().insert(node_id.clone());
    }
    GroupAck { groups: join.groups.clone() }
  }

  fn leave_groups(&mut self, node_id: &NodeId, leave: &GroupLeave) -> GroupAck {
    for group in &leave.groups {
      if let Some(members) = self.groups.get_mut(group) {
        members.remove(node_id);
        if members.is_empty() {
          self.groups.remove(group);
        }
      }
    }
    GroupAck { groups: leave.groups.clone() }
  }

  fn lookup_agent(&self, agent_id: &AgentId) -> LookupAgentResult {
    LookupAgentResult {
      agent_id: agent_id.clone(),
      node_id: self.agent_routes.get(agent_id).cloned(),
    }
  }

  fn route_envelope(&mut self, _source_node_id: &NodeId, envelope: WireEnvelope) -> RouteOutcome {
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
        let Recipient::Group(group_name) = &envelope.meta.recipient else { unreachable!() };
        let Some(members) = self.groups.get(group_name).cloned() else {
          self.unknown_recipient_drops += 1;
          outcome.drop_reason = Some("unknown_group".to_string());
          return outcome;
        };

        let frame = WireFrame::Envelope(envelope);
        for target_node_id in members {
          let Some(peer) = self.peers.get(&target_node_id) else {
            continue;
          };
          if peer.outbound_tx.try_send(frame.clone()).is_err() {
            self.backpressure_drops += 1;
            outcome.backpressure_drops += 1;
          } else {
            outcome.recipients.push(target_node_id);
          }
        }

        if outcome.recipients.is_empty() && outcome.backpressure_drops > 0 {
          outcome.drop_reason = Some("backpressure".to_string());
        }
      },
    }
    outcome
  }

  fn remove_peer(&mut self, node_id: &NodeId) {
    self.peers.remove(node_id);
    self.agent_routes.retain(|_, owner| owner != node_id);
    self.groups.retain(|_, members| {
      members.remove(node_id);
      !members.is_empty()
    });
  }

  fn is_peer_timed_out(&self, node_id: &NodeId, timeout: Duration) -> bool {
    self.peers.get(node_id).is_some_and(|peer| peer.last_seen.elapsed() > timeout)
  }
}

async fn run_connection(
  mut transport: FramedTcpStream,
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

  #[derive(Debug)]
  enum IoEvent {
    Frame(WireFrame),
    PeerClosed,
    ReadError(TcpTransportError),
    WriteError(TcpTransportError),
  }

  let (mut reader, mut writer) = transport.into_split();
  let (io_event_tx, mut io_event_rx) =
    mpsc::channel::<IoEvent>(config.outbound_queue_capacity.max(4));
  let io_event_tx_reader = io_event_tx.clone();
  let reader_task = tokio::spawn(async move {
    loop {
      match reader.recv_frame().await {
        Ok(Some(frame)) => {
          if io_event_tx_reader.send(IoEvent::Frame(frame)).await.is_err() {
            break;
          }
        },
        Ok(None) => {
          let _ = io_event_tx_reader.send(IoEvent::PeerClosed).await;
          break;
        },
        Err(error) => {
          let _ = io_event_tx_reader.send(IoEvent::ReadError(error)).await;
          break;
        },
      }
    }
  });
  let io_event_tx_writer = io_event_tx.clone();
  let writer_task = tokio::spawn(async move {
    while let Some(frame) = outbound_rx.recv().await {
      if let Err(error) = writer.send_frame(&frame).await {
        let _ = io_event_tx_writer.send(IoEvent::WriteError(error)).await;
        break;
      }
    }
  });
  drop(io_event_tx);

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
      io_event = io_event_rx.recv() => {
        let Some(io_event) = io_event else {
          break "io_task_closed".to_string();
        };

        let frame = match io_event {
          IoEvent::Frame(frame) => frame,
          IoEvent::PeerClosed => break "peer_closed".to_string(),
          IoEvent::ReadError(error) => break format!("read_error:{error}"),
          IoEvent::WriteError(error) => break format!("write_error:{error}"),
        };

        let mut state = state.lock().await;
        state.touch_peer(&node_id);

        match frame {
          WireFrame::Heartbeat(_) => {}
          WireFrame::AdvertiseAgents(advertise) => {
            let ack = state.register_agents(&node_id, &advertise);
            let agent_count = advertise.agents.len();
            drop(state);
            if outbound_tx.send(WireFrame::AdvertiseAck(ack)).await.is_err() {
              break "writer_closed".to_string();
            }
            if let Some(observer) = &observer {
              observer.emit(ObserveEventKind::BrokerAgentsAdvertised {
                node_id: node_id.clone(),
                agent_count,
              });
            }
          }
          WireFrame::GroupJoin(join) => {
            let ack = state.join_groups(&node_id, &join);
            drop(state);
            if outbound_tx.send(WireFrame::GroupAck(ack)).await.is_err() {
              break "writer_closed".to_string();
            }
          }
          WireFrame::GroupLeave(leave) => {
            let ack = state.leave_groups(&node_id, &leave);
            drop(state);
            if outbound_tx.send(WireFrame::GroupAck(ack)).await.is_err() {
              break "writer_closed".to_string();
            }
          }
          WireFrame::LookupAgent(lookup) => {
            let result = state.lookup_agent(&lookup.agent_id);
            drop(state);
            if outbound_tx.send(WireFrame::LookupAgentResult(result)).await.is_err() {
              break "writer_closed".to_string();
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
          | WireFrame::AdvertiseAck(_)
          | WireFrame::GroupAck(_)
          | WireFrame::LookupAgentResult(_) => {}
        }
      }
    }
  };

  let mut state = state.lock().await;
  state.remove_peer(&node_id);
  drop(state);
  drop(outbound_tx);
  writer_task.abort();
  reader_task.abort();
  let _ = writer_task.await;
  let _ = reader_task.await;
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
    protocol::{
      AgentId, CorrelationId, EnvelopeMeta, HandshakeHello, Recipient, WireEnvelope, capabilities,
    },
    runtime::node::{NodeClient, NodeRequestError},
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

  #[tokio::test]
  async fn node_client_exposes_broker_coordination_capabilities_from_handshake() {
    let broker = spawn(
      BrokerConfig::new(SocketAddr::from(([127, 0, 0, 1], 0)), "broker-1")
        .with_coordination_capabilities(),
    )
    .await
    .expect("spawn broker");

    let node = connect_node(&broker, "node-a").await;
    assert!(node.broker_supports_capability(capabilities::GROUP_ROUTING_V1));
    assert!(node.broker_supports_capability(capabilities::AGENT_LOOKUP_V1));
    assert!(node.broker_supports_capability(capabilities::REQUEST_REPLY_V1));
    assert!(!node.broker_supports_capability("coord.unknown.v1"));

    drop(node);
    broker.shutdown().await.expect("shutdown broker");
  }

  #[tokio::test]
  async fn broker_routes_group_messages_and_honors_group_leave() {
    let broker = spawn_test_broker().await;

    let mut node_a = connect_node(&broker, "node-a").await;
    let mut node_b = connect_node(&broker, "node-b").await;
    let mut node_c = connect_node(&broker, "node-c").await;

    node_b.join_groups(vec!["workers".to_string()]).await.expect("join group b");
    node_c.join_groups(vec!["workers".to_string()]).await.expect("join group c");

    let group_envelope =
      WireEnvelope::new(EnvelopeMeta::new("example.group").to_group("workers"), vec![4, 2]);
    node_a.send_envelope(group_envelope.clone()).await.expect("send group envelope");

    let b_frame = timeout(Duration::from_secs(1), node_b.recv_frame())
      .await
      .expect("node b recv timeout")
      .expect("node b recv result")
      .expect("node b frame");
    let c_frame = timeout(Duration::from_secs(1), node_c.recv_frame())
      .await
      .expect("node c recv timeout")
      .expect("node c recv result")
      .expect("node c frame");
    for frame in [b_frame, c_frame] {
      match frame {
        WireFrame::Envelope(actual) => assert_eq!(actual, group_envelope),
        other => panic!("unexpected frame: {other:?}"),
      }
    }
    assert!(
      timeout(Duration::from_millis(100), node_a.recv_frame()).await.is_err(),
      "sender should not receive group message without membership"
    );

    node_c.leave_groups(vec!["workers".to_string()]).await.expect("leave group c");
    let post_leave =
      WireEnvelope::new(EnvelopeMeta::new("example.group").to_group("workers"), vec![9, 9, 9]);
    node_a.send_envelope(post_leave.clone()).await.expect("send post-leave group envelope");

    let b_frame = timeout(Duration::from_secs(1), node_b.recv_frame())
      .await
      .expect("node b recv timeout (post leave)")
      .expect("node b recv result (post leave)")
      .expect("node b frame (post leave)");
    match b_frame {
      WireFrame::Envelope(actual) => assert_eq!(actual, post_leave),
      other => panic!("unexpected frame: {other:?}"),
    }
    assert!(
      timeout(Duration::from_millis(150), node_c.recv_frame()).await.is_err(),
      "node c should not receive group frames after leaving"
    );

    drop(node_a);
    drop(node_b);
    drop(node_c);
    broker.shutdown().await.expect("shutdown broker");
  }

  #[tokio::test]
  async fn broker_lookup_agent_returns_registered_owner_node() {
    let broker = spawn_test_broker().await;

    let mut node_a = connect_node(&broker, "node-a").await;
    let mut node_b = connect_node(&broker, "node-b").await;
    node_b.advertise_agents(vec![AgentId::from("agent-b")]).await.expect("advertise agents");

    let found = node_a.lookup_agent("agent-b").await.expect("lookup registered");
    assert_eq!(found.agent_id.as_str(), "agent-b");
    assert_eq!(found.node_id.as_ref().map(NodeId::as_str), Some("node-b"));

    let missing = node_a.lookup_agent("missing-agent").await.expect("lookup missing");
    assert_eq!(missing.agent_id.as_str(), "missing-agent");
    assert_eq!(missing.node_id, None);

    drop(node_a);
    drop(node_b);
    broker.shutdown().await.expect("shutdown broker");
  }

  #[tokio::test]
  async fn node_request_reply_helper_matches_correlation_and_buffers_unrelated_frames() {
    let broker = spawn_test_broker().await;

    let mut node_a = connect_node(&broker, "node-a").await;
    let mut node_b = connect_node(&broker, "node-b").await;
    node_a.advertise_agents(vec![AgentId::from("agent-a")]).await.expect("advertise agent a");
    node_b.advertise_agents(vec![AgentId::from("agent-b")]).await.expect("advertise agent b");

    let responder = tokio::spawn(async move {
      let request = timeout(Duration::from_secs(1), node_b.recv_frame())
        .await
        .expect("node b recv timeout")
        .expect("node b recv result")
        .expect("node b frame");

      let WireFrame::Envelope(request) = request else {
        panic!("expected envelope request");
      };
      assert_eq!(request.meta.recipient, Recipient::Agent(AgentId::from("agent-b")));

      let stray =
        WireEnvelope::new(EnvelopeMeta::new("example.stray").to_agent("agent-a"), vec![1]);
      node_b.send_envelope(stray).await.expect("send stray envelope");

      let reply = WireEnvelope::new(
        EnvelopeMeta::new("example.reply")
          .to_agent("agent-a")
          .with_correlation_id(CorrelationId::from(request.meta.message_id)),
        vec![2, 3, 4],
      );
      node_b.send_envelope(reply).await.expect("send reply envelope");

      node_b
    });

    let request =
      WireEnvelope::new(EnvelopeMeta::new("example.request").to_agent("agent-b"), vec![9]);
    let expected_correlation = CorrelationId::from(request.meta.message_id);
    let reply = node_a
      .request_envelope_with_timeout(request, Duration::from_millis(500))
      .await
      .expect("request reply success");
    assert_eq!(reply.payload, vec![2, 3, 4]);
    assert_eq!(reply.meta.correlation_id, Some(expected_correlation));
    assert_eq!(reply.meta.recipient, Recipient::Agent(AgentId::from("agent-a")));

    let buffered = timeout(Duration::from_secs(1), node_a.recv_frame())
      .await
      .expect("buffered frame timeout")
      .expect("buffered frame recv result")
      .expect("buffered frame");
    match buffered {
      WireFrame::Envelope(envelope) => {
        assert_eq!(envelope.meta.message_kind.as_str(), "example.stray");
        assert_eq!(envelope.payload, vec![1]);
      },
      other => panic!("unexpected frame: {other:?}"),
    }

    let node_b = responder.await.expect("responder task");
    drop(node_b);
    drop(node_a);
    broker.shutdown().await.expect("shutdown broker");
  }

  #[tokio::test]
  async fn node_request_reply_helper_times_out_without_matching_reply() {
    let broker = spawn_test_broker().await;

    let mut node_a = connect_node(&broker, "node-a").await;
    let request =
      WireEnvelope::new(EnvelopeMeta::new("example.request").to_agent("missing-agent"), vec![5]);
    let error = node_a
      .request_envelope_with_timeout(request, Duration::from_millis(75))
      .await
      .expect_err("request should time out");

    match error {
      NodeRequestError::Timeout { timeout } => assert_eq!(timeout, Duration::from_millis(75)),
      other => panic!("unexpected request error: {other:?}"),
    }

    drop(node_a);
    broker.shutdown().await.expect("shutdown broker");
  }
}
