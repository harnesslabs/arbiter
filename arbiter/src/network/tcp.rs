//! TCP network implementation for distributed actor communication.

use std::{any::TypeId, collections::HashMap, fmt::Debug, net::SocketAddr};

use tokio::{
  io::{AsyncReadExt, AsyncWriteExt},
  net::{TcpListener, TcpStream as TokioTcpStream},
  sync::mpsc,
};

use crate::{
  handler::{Envelope, Message},
  network::{Network, Socket, registry::global_registry},
};

/// An envelope format used for sending messages over the wire.
/// It uses a string representation of the type to remain stable
/// across different compiler runs and binary versions.
#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct WireEnvelope {
  type_name: String,
  payload: Vec<u8>,
}

/// An envelope containing a message for the `TcpStream` network.
///
/// Unlike the in-memory envelope, `TcpEnvelope` stores the message as a serialized
/// byte buffer alongside its type name, allowing it to be transmitted across
/// process boundaries.
#[derive(Clone)]
pub struct TcpEnvelope {
  /// The `TypeId` of the message, used for local routing.
  pub type_id: TypeId,
  /// The string name of the type, used for remote routing.
  pub type_name: String,
  /// The serialized message payload.
  pub payload: Vec<u8>,
}

impl Debug for TcpEnvelope {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "TcpEnvelope {{ type_name: {} }}", self.type_name)
  }
}

impl Envelope for TcpEnvelope {
  fn type_id(&self) -> TypeId {
    self.type_id
  }

  fn register_type<M: Message>() {
    global_registry().register::<M>();
  }

  fn wrap<M: Message>(message: M) -> Self {
    global_registry().register::<M>();
    let type_name = std::any::type_name::<M>().to_string();
    let payload = bincode::serialize(&message).expect("Failed to serialize message");
    Self { type_id: TypeId::of::<M>(), type_name, payload }
  }

  fn downcast<M: Message>(&self) -> Option<impl std::ops::Deref<Target = M> + '_> {
    if self.type_id == TypeId::of::<M>() {
      match bincode::deserialize::<M>(&self.payload) {
        Ok(msg) => Some(Box::new(msg)),
        Err(e) => {
          tracing::error!("Failed to deserialize message: {}", e);
          None
        },
      }
    } else {
      None
    }
  }
}

/// A network address for the `TcpStream` backend.
///
/// Identification consists of the originating node's public `SocketAddr`
/// and a unique `actor_id` assigned by that node's local runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TcpAddress {
  /// The IP and port of the physical node.
  pub node: SocketAddr,
  /// The unique identifier for the actor on that node.
  pub actor_id: u64,
}

impl std::fmt::Display for TcpAddress {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}#{}", self.node, self.actor_id)
  }
}

impl TcpAddress {
  fn generate(node: SocketAddr) -> Self {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    Self { node, actor_id: COUNTER.fetch_add(1, Ordering::Relaxed) }
  }
}

enum RouterMessage {
  Register(TcpAddress, mpsc::UnboundedSender<TcpEnvelope>),
  Subscribe(TcpAddress, TypeId),
  DispatchLocal(TcpEnvelope),
  ConnectTo(SocketAddr),
  IncomingRemote(SocketAddr, WireEnvelope),
  ConnectionEstablished(SocketAddr, mpsc::UnboundedSender<WireEnvelope>),
  ConnectionFailed(SocketAddr),
}

fn spawn_connection(
  stream: TokioTcpStream,
  mut out_rx: mpsc::UnboundedReceiver<WireEnvelope>,
  in_tx: mpsc::UnboundedSender<RouterMessage>,
  peer_addr: SocketAddr,
) {
  tokio::spawn(async move {
    let (mut rd, mut wr) = stream.into_split();

    let mut write_task = tokio::spawn(async move {
      tracing::debug!("Started write_task for {}", peer_addr);
      while let Some(wire) = out_rx.recv().await {
        if let Ok(bytes) = bincode::serialize(&wire) {
          if let Err(e) = wr.write_u32(bytes.len() as u32).await {
            tracing::error!("Failed to write len to {}: {}", peer_addr, e);
            break;
          }
          if let Err(e) = wr.write_all(&bytes).await {
            tracing::error!("Failed to write payload to {}: {}", peer_addr, e);
            break;
          }
          tracing::debug!("Successfully wrote envelope '{}' to {}", wire.type_name, peer_addr);
        } else {
          tracing::error!("Failed to serialize wire envelope");
        }
      }
      tracing::debug!("write_task ended for {}", peer_addr);
    });

    let mut read_task = tokio::spawn(async move {
      tracing::debug!("Started read_task for {}", peer_addr);
      loop {
        match rd.read_u32().await {
          Ok(len) => {
            let mut buf = vec![0; len as usize];
            if let Err(e) = rd.read_exact(&mut buf).await {
              tracing::error!("Failed to read exact buf from {}: {}", peer_addr, e);
              break;
            }
            match bincode::deserialize::<WireEnvelope>(&buf) {
              Ok(wire) => {
                tracing::debug!(
                  "Successfully read envelope '{}' from {}",
                  wire.type_name,
                  peer_addr
                );
                let _ = in_tx.send(RouterMessage::IncomingRemote(peer_addr, wire));
              },
              Err(e) => {
                tracing::warn!("Failed to deserialize wire envelope from {}: {}", peer_addr, e);
              },
            }
          },
          Err(e) => {
            tracing::debug!("read_u32 failed/closed for {}: {}", peer_addr, e);
            break;
          },
        }
      }
      tracing::debug!("read_task ended for {}", peer_addr);
    });

    tokio::select! {
      _ = &mut write_task => read_task.abort(),
      _ = &mut read_task => write_task.abort(),
    }
  });
}

async fn router(
  listener: TcpListener,
  mut rx: mpsc::UnboundedReceiver<RouterMessage>,
  local_addr: SocketAddr,
) {
  let mut inboxes: HashMap<TcpAddress, mpsc::UnboundedSender<TcpEnvelope>> = HashMap::new();
  let mut routes: HashMap<TypeId, Vec<TcpAddress>> = HashMap::new();

  // Maps a verified remote listener address (from handshake) to a connection
  let mut remotes: HashMap<SocketAddr, mpsc::UnboundedSender<WireEnvelope>> = HashMap::new();

  // Maps an ephemeral incoming connection to a connection
  let mut unverified_remotes: HashMap<SocketAddr, mpsc::UnboundedSender<WireEnvelope>> =
    HashMap::new();

  // Tracks all known active nodes in the mesh
  let mut mesh_peers: std::collections::HashSet<SocketAddr> = std::collections::HashSet::new();
  mesh_peers.insert(local_addr);

  // Tracks connection attempts to avoid duplicating TCP handshake starts
  let mut pending_connections: std::collections::HashSet<SocketAddr> =
    std::collections::HashSet::new();

  let (remote_in_tx, mut remote_in_rx) = mpsc::unbounded_channel::<RouterMessage>();

  loop {
    tokio::select! {
      Ok((stream, peer_addr)) = listener.accept() => {
        tracing::info!("Accepted incoming connection from {}", peer_addr);
        let (tx, rx) = mpsc::unbounded_channel();
        unverified_remotes.insert(peer_addr, tx);
        spawn_connection(stream, rx, remote_in_tx.clone(), peer_addr);
      }
      Some(msg) = remote_in_rx.recv() => {
        match msg {
          RouterMessage::IncomingRemote(addr, wire) => {
            if wire.type_name == "$arbiter::Handshake" {
              if let Ok(remote_listener_addr) = bincode::deserialize::<SocketAddr>(&wire.payload) {
                // Determine if we should keep this connection
                let should_keep = if remotes.contains_key(&remote_listener_addr) {
                  // Wait, actually, let's simplify.
                  // If we already have a connection to them in `remotes`, it must be an outgoing connection we initiated,
                  // or another completed incoming connection.
                  // Let's standardise: Higher `SocketAddr` wins tie-breaker and its connection is kept.
                  if local_addr > remote_listener_addr {
                    tracing::info!("Tie-break: local {} > remote {}. Keeping existing connection, rejecting new incoming.", local_addr, remote_listener_addr);
                    false
                  } else {
                    tracing::info!("Tie-break: local {} <= remote {}. Replacing existing with new incoming.", local_addr, remote_listener_addr);
                    true
                  }
                } else {
                  tracing::info!("Incoming handshake from new peer: {}", remote_listener_addr);
                  true
                };

                if should_keep {
                  if let Some(tx) = unverified_remotes.remove(&addr) {
                    remotes.insert(remote_listener_addr, tx.clone());
                    mesh_peers.insert(remote_listener_addr);

                    // Reply with our MeshPeers
                    let peers: Vec<SocketAddr> = mesh_peers.iter().copied().collect();
                    let reply = WireEnvelope {
                      type_name: "$arbiter::MeshPeers".to_string(),
                      payload: bincode::serialize(&peers).unwrap(),
                    };
                    let _ = tx.send(reply);

                    tracing::info!("Handshake complete. Client {} verified as listener {}", addr, remote_listener_addr);
                  }
                }
              }
            } else if wire.type_name == "$arbiter::MeshPeers" {
              if let Ok(peers) = bincode::deserialize::<Vec<SocketAddr>>(&wire.payload) {
                let mut new_peers = false;
                for peer in peers {
                  if mesh_peers.insert(peer) {
                    tracing::info!("Discovered new mesh peer: {}", peer);
                    new_peers = true;
                    // Connect to new peer
                    let _ = remote_in_tx.send(RouterMessage::ConnectTo(peer));
                  }
                }
                if new_peers {
                  // Gossip to all our verified remotes
                  let peers_vec: Vec<SocketAddr> = mesh_peers.iter().copied().collect();
                  let gossip = WireEnvelope {
                    type_name: "$arbiter::MeshPeers".to_string(),
                    payload: bincode::serialize(&peers_vec).unwrap(),
                  };
                  for tx in remotes.values() {
                    let _ = tx.send(gossip.clone());
                  }
                }
              }
            } else if let Some(type_id) = global_registry().get_id(&wire.type_name) {
              let env = TcpEnvelope { type_id, type_name: wire.type_name, payload: wire.payload };
              if let Some(addrs) = routes.get(&type_id) {
                for act_addr in addrs {
                  if let Some(inbox) = inboxes.get(act_addr) {
                    let _ = inbox.send(env.clone());
                  }
                }
              }
            } else {
              tracing::warn!("Received unregistered message type '{}' from {}", wire.type_name, addr);
            }
          }
          RouterMessage::ConnectionEstablished(remote_listener_addr, tx) => {
            pending_connections.remove(&remote_listener_addr);
            remotes.insert(remote_listener_addr, tx.clone());
            mesh_peers.insert(remote_listener_addr);

            // Send Handshake
            let handshake = WireEnvelope {
              type_name: "$arbiter::Handshake".to_string(),
              payload: bincode::serialize(&local_addr).unwrap(),
            };
            let _ = tx.send(handshake);

            // Send MeshPeers
            let peers_vec: Vec<SocketAddr> = mesh_peers.iter().copied().collect();
            let gossip = WireEnvelope {
              type_name: "$arbiter::MeshPeers".to_string(),
              payload: bincode::serialize(&peers_vec).unwrap(),
            };
            let _ = tx.send(gossip);
          }
          RouterMessage::ConnectionFailed(addr) => {
            pending_connections.remove(&addr);
          }
          RouterMessage::ConnectTo(addr) => {
            // Check if we are already connected, attempting to connect, or if it is ourself
            if addr == local_addr || remotes.contains_key(&addr) || pending_connections.contains(&addr) {
              tracing::debug!("Skipping ConnectTo({}): already connected, pending, or self", addr);
              continue;
            }
            pending_connections.insert(addr);
            tracing::info!("Initiating ConnectTo: {}", addr);
            let remote_in_tx_clone = remote_in_tx.clone();
            tokio::spawn(async move {
              match tokio::net::TcpStream::connect(addr).await {
                Ok(stream) => {
                  tracing::info!("Connected to remote node at {}", addr);
                  let (tx, rx) = mpsc::unbounded_channel();
                  let _ = remote_in_tx_clone.send(RouterMessage::ConnectionEstablished(addr, tx));
                  spawn_connection(stream, rx, remote_in_tx_clone, addr);
                }
                Err(e) => {
                  tracing::error!("Failed to connect to remote node at {}: {}", addr, e);
                  let _ = remote_in_tx_clone.send(RouterMessage::ConnectionFailed(addr));
                }
              }
            });
          }
          _ => {}
        }
      }
      Some(msg) = rx.recv() => {
        match msg {
          RouterMessage::Register(addr, tx) => { inboxes.insert(addr, tx); },
          RouterMessage::Subscribe(addr, tid) => { routes.entry(tid).or_default().push(addr); },
          RouterMessage::DispatchLocal(env) => {
            // Local dispatch
            if let Some(addrs) = routes.get(&env.type_id) {
              for addr in addrs {
                if let Some(inbox) = inboxes.get(addr) {
                  let _ = inbox.send(env.clone());
                }
              }
            }
            // Broadcast
            let wire = WireEnvelope { type_name: env.type_name.clone(), payload: env.payload.clone() };
            for rx_tx in remotes.values() {
              let _ = rx_tx.send(wire.clone());
            }
          }
          RouterMessage::ConnectTo(addr) => {
            let _ = remote_in_tx.send(RouterMessage::ConnectTo(addr));
          }
          _ => {}
        }
      }
    }
  }
}

/// A network implementation that communicates via TCP streams.
///
/// `TcpStream` enables actors to communicate across a LAN or the internet.
/// It maintains a background router task that manages persistent connections
/// to other nodes and handles message serialization/deserialization.
#[derive(Debug)]
pub struct TcpStream {
  router_tx: mpsc::UnboundedSender<RouterMessage>,
  local_addr: SocketAddr,
}

impl TcpStream {
  /// Resolves an address and connects the router to it.
  ///
  /// # Errors
  ///
  /// Returns an error if the host cannot be statically resolved.
  ///
  /// # Panics
  ///
  /// Panics if the resolved address yields zero results.
  pub async fn connect_to(&self, addr: impl tokio::net::ToSocketAddrs) -> std::io::Result<()> {
    let addrs = tokio::net::lookup_host(addr).await?;
    if let Some(resolved) = addrs.into_iter().next() {
      let _ = self.router_tx.send(RouterMessage::ConnectTo(resolved));
      Ok(())
    } else {
      Err(std::io::Error::new(std::io::ErrorKind::AddrNotAvailable, "Failed to resolve address"))
    }
  }

  /// Returns the randomly assigned local bind port / IP.
  #[must_use]
  pub const fn local_addr(&self) -> SocketAddr {
    self.local_addr
  }
}

impl Network for TcpStream {
  type Socket = TcpSocket;

  fn new() -> Self {
    let std_listener =
      std::net::TcpListener::bind("0.0.0.0:0").expect("Failed to bind TCP listener");
    std_listener.set_nonblocking(true).unwrap();
    let listener = TcpListener::from_std(std_listener).unwrap();
    let mut local_addr = listener.local_addr().unwrap();
    // Resolve the actual LAN IP for handshakes rather than 0.0.0.0 or localhost
    if local_addr.ip().is_unspecified() {
      if let Ok(ip) = local_ip_address::local_ip() {
        local_addr.set_ip(ip);
      } else {
        local_addr.set_ip(std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)));
      }
    }

    let (router_tx, router_rx) = mpsc::unbounded_channel();

    let router_rx_local_addr = local_addr;
    tokio::spawn(router(listener, router_rx, router_rx_local_addr));

    Self { router_tx, local_addr }
  }

  fn connect(&mut self) -> Self::Socket {
    let address = TcpAddress::generate(self.local_addr);
    let (inbox_tx, inbox_rx) = mpsc::unbounded_channel();
    let _ = self.router_tx.send(RouterMessage::Register(address, inbox_tx));
    TcpSocket { address, router_tx: self.router_tx.clone(), inbox_rx }
  }

  fn subscribe(&self, address: <Self::Socket as Socket>::Address, type_id: TypeId) {
    let _ = self.router_tx.send(RouterMessage::Subscribe(address, type_id));
  }
}

/// The socket endpoint assigned to an actor on the `TcpStream` network.
pub struct TcpSocket {
  address: TcpAddress,
  router_tx: mpsc::UnboundedSender<RouterMessage>,
  inbox_rx: mpsc::UnboundedReceiver<TcpEnvelope>,
}

impl Debug for TcpSocket {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "TcpSocket {{ address: {} }}", self.address)
  }
}

impl Socket for TcpSocket {
  type Address = TcpAddress;
  type Envelope = TcpEnvelope;

  fn address(&self) -> Self::Address {
    self.address
  }

  async fn send(&self, envelope: Self::Envelope) {
    let _ = self.router_tx.send(RouterMessage::DispatchLocal(envelope));
  }

  async fn receive(&mut self) -> Option<Self::Envelope> {
    self.inbox_rx.recv().await
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::fixtures::{Ping, Pong};

  #[test]
  fn test_tcp_envelope_wrap_and_downcast() {
    let ping = Ping;
    let env = TcpEnvelope::wrap(ping);

    assert_eq!(env.type_id(), TypeId::of::<Ping>());
    assert_eq!(env.type_name, std::any::type_name::<Ping>());

    // Successful downcast
    let downcasted = env.downcast::<Ping>();
    assert!(downcasted.is_some());

    // Failed downcast
    let bad_downcast = env.downcast::<Pong>();
    assert!(bad_downcast.is_none());
  }
}
