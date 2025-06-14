use std::{future::Future, hash::Hash, pin::Pin};

use crate::handler::{Envelope, Message, Package};

#[cfg(feature = "in-memory")] pub mod memory;

#[cfg(feature = "tcp")] pub mod tcp;

pub trait Runtime: 'static {
  type Output<T>;
  fn wrap<T: 'static>(value: T) -> Self::Output<T>;
}

pub struct SyncRuntime;

impl Runtime for SyncRuntime {
  type Output<T> = T;

  fn wrap<T: 'static>(value: T) -> Self::Output<T> { value }
}

pub struct AsyncRuntime;

impl Runtime for AsyncRuntime {
  type Output<T> = Pin<Box<dyn Future<Output = T>>>;

  fn wrap<T: 'static>(value: T) -> Self::Output<T> { Box::pin(async move { value }) }
}

pub trait Generateable {
  fn generate() -> Self;
}

#[derive(Debug)]
pub struct Connection<T: Transport> {
  pub address:   T::Address,
  pub transport: T,
}

impl<T: Transport> Connection<T> {
  pub fn new(address: T::Address) -> Self {
    let channel = T::spawn();
    Self { address, transport: channel }
  }

  pub fn join(&self) -> Self {
    let channel = self.transport.join();
    Self { address: self.address, transport: channel }
  }
}

pub trait Transport: Send + Sync + Sized + 'static {
  type Address: Generateable
    + Copy
    + Send
    + Sync
    + PartialEq
    + Eq
    + Hash
    + std::fmt::Debug
    + std::fmt::Display;
  type Payload: Clone + Message + Package<Self::Payload>;

  fn spawn() -> Self;
  fn join(&self) -> Self;
  fn send(&self, envelope: Envelope<Self>);
  fn receive(&self) -> Option<Envelope<Self>>;
}

// pub struct Transport<C: Connection> {
//   pub(crate) inbound_connection: C,
// }

// // TODO: These should return results
// impl<C: Connection> Transport<C> {
//   pub fn new() -> Self {
//     Self {
//       inbound_connection:   C::new(),
//       outbound_connections: Arc::new(Mutex::new(HashMap::new())),
//     }
//   }

//   pub fn send(&mut self, envelope: Envelope<C>, address: C::Address) {
//     self.outbound_connections.lock().unwrap().get_mut(&address).map(|sender|
// sender.send(envelope));   }

//   pub fn broadcast(&mut self, envelope: Envelope<C>) {
//     self
//       .outbound_connections
//       .lock()
//       .unwrap()
//       .values_mut()
//       .for_each(|sender| sender.send(envelope.clone()));
//   }

//   pub fn receive(&mut self) -> Option<Envelope<C>> { self.inbound_connection.receiver().receive()
// }

//   pub fn add_outbound_connection(&mut self, address: C::Address, sender: C::Sender) {
//     self.outbound_connections.lock().unwrap().insert(address, sender);
//   }

//   pub fn create_inbound_connection(&self) -> (C::Address, C::Sender) {
//     self.inbound_connection.create_outbound_connection()
//   }
// }
