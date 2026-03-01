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
  payload:   Vec<u8>,
}

/// An envelope containing a message for the `TcpStream` network.
///
/// Unlike the in-memory envelope, `TcpEnvelope` stores the message as a serialized
/// byte buffer alongside its type name, allowing it to be transmitted across
/// process boundaries.
#[derive(Clone)]
pub struct TcpEnvelope {
  /// The `TypeId` of the message, used for local routing.
  pub type_id:   TypeId,
  /// The string name of the type, used for remote routing.
  pub type_name: String,
  /// The serialized message payload.
  pub payload:   Vec<u8>,
}

impl Debug for TcpEnvelope {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "TcpEnvelope {{ type_name: {} }}", self.type_name)
  }
}

impl Envelope for TcpEnvelope {
  fn type_id(&self) -> TypeId { self.type_id }

  fn register_type<M: Message>() { global_registry().register::<M>(); }

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
  pub node:     SocketAddr,
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
  AddRemote(SocketAddr, mpsc::UnboundedSender<WireEnvelope>),
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
      while let Some(wire) = out_rx.recv().await {
        if let Ok(bytes) = bincode::serialize(&wire) {
          if wr.write_u32(bytes.len() as u32).await.is_err() {
            break;
          }
          if wr.write_all(&bytes).await.is_err() {
            break;
          }
        }
      }
    });

    let mut read_task = tokio::spawn(async move {
      while let Ok(len) = rd.read_u32().await {
        let mut buf = vec![0; len as usize];
        if rd.read_exact(&mut buf).await.is_err() {
          break;
        }
        match bincode::deserialize::<WireEnvelope>(&buf) {
          Ok(wire) => {
            let _ = in_tx.send(RouterMessage::IncomingRemote(peer_addr, wire));
          },
          Err(e) => {
            tracing::warn!("Failed to deserialize wire envelope from {}: {}", peer_addr, e);
          },
        }
      }
    });

    tokio::select! {
      _ = &mut write_task => read_task.abort(),
      _ = &mut read_task => write_task.abort(),
    }
  });
}

async fn router(listener: TcpListener, mut rx: mpsc::UnboundedReceiver<RouterMessage>) {
  let mut inboxes: HashMap<TcpAddress, mpsc::UnboundedSender<TcpEnvelope>> = HashMap::new();
  let mut routes: HashMap<TypeId, Vec<TcpAddress>> = HashMap::new();
  let mut remotes: HashMap<SocketAddr, mpsc::UnboundedSender<WireEnvelope>> = HashMap::new();

  let (remote_in_tx, mut remote_in_rx) = mpsc::unbounded_channel::<RouterMessage>();

  loop {
    tokio::select! {
      Ok((stream, peer_addr)) = listener.accept() => {
        tracing::info!("Accepted incoming connection from {}", peer_addr);
        let (tx, rx) = mpsc::unbounded_channel();
        remotes.insert(peer_addr, tx);
        spawn_connection(stream, rx, remote_in_tx.clone(), peer_addr);
      }
      Some(msg) = remote_in_rx.recv() => {
        match msg {
          RouterMessage::IncomingRemote(addr, wire) => {
            if let Some(type_id) = global_registry().get_id(&wire.type_name) {
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
          RouterMessage::AddRemote(addr, tx) => { remotes.insert(addr, tx); }
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
            let remote_in_tx = remote_in_tx.clone();
            tokio::spawn(async move {
              match tokio::net::TcpStream::connect(addr).await {
                Ok(stream) => {
                  tracing::info!("Connected to remote node at {}", addr);
                  let (tx, rx) = mpsc::unbounded_channel();
                  let _ = remote_in_tx.send(RouterMessage::AddRemote(addr, tx));
                  spawn_connection(stream, rx, remote_in_tx, addr);
                }
                Err(e) => {
                  tracing::error!("Failed to connect to remote node at {}: {}", addr, e);
                }
              }
            });
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
  router_tx:  mpsc::UnboundedSender<RouterMessage>,
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
  pub const fn local_addr(&self) -> SocketAddr { self.local_addr }
}

impl Network for TcpStream {
  type Socket = TcpSocket;

  fn new() -> Self {
    let std_listener =
      std::net::TcpListener::bind("0.0.0.0:0").expect("Failed to bind TCP listener");
    std_listener.set_nonblocking(true).unwrap();
    let listener = TcpListener::from_std(std_listener).unwrap();
    let local_addr = listener.local_addr().unwrap();

    let (router_tx, router_rx) = mpsc::unbounded_channel();

    tokio::spawn(router(listener, router_rx));

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
  address:   TcpAddress,
  router_tx: mpsc::UnboundedSender<RouterMessage>,
  inbox_rx:  mpsc::UnboundedReceiver<TcpEnvelope>,
}

impl Debug for TcpSocket {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "TcpSocket {{ address: {} }}", self.address)
  }
}

impl Socket for TcpSocket {
  type Address = TcpAddress;
  type Envelope = TcpEnvelope;

  fn address(&self) -> Self::Address { self.address }

  async fn send(&self, envelope: Self::Envelope) {
    let _ = self.router_tx.send(RouterMessage::DispatchLocal(envelope));
  }

  async fn receive(&mut self) -> Option<Self::Envelope> { self.inbox_rx.recv().await }
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
