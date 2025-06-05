use std::{collections::HashMap, future::Future, hash::Hash, pin::Pin};

use crate::handler::{Envelope, Message, Package};

pub mod memory;
pub mod tcp;

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

// TODO: Need to have results for send and receive.
pub trait Sender: Send + Sync + 'static {
  type Connection: Connection;
  fn send(&self, envelope: Envelope<Self::Connection>);
}

pub trait Receiver: Send + Sync + 'static {
  type Connection: Connection;
  fn receive(&self) -> Option<Envelope<Self::Connection>>;
}

pub trait Connection: Send + Sync + 'static {
  type Address: Copy + Send + Sync + PartialEq + Eq + Hash + std::fmt::Debug + std::fmt::Display;
  type Payload: Clone + Message + Package<Self::Payload>;
  type Sender: Sender<Connection = Self>;
  type Receiver: Receiver<Connection = Self>;

  fn new() -> Self;

  fn address(&self) -> Self::Address;

  fn sender(&mut self) -> &mut Self::Sender;

  fn receiver(&mut self) -> &mut Self::Receiver;

  fn create_outbound_connection(&self) -> (Self::Address, Self::Sender);
}

pub struct Transport<C: Connection> {
  pub(crate) inbound_connection:   C,
  pub(crate) outbound_connections: HashMap<C::Address, C::Sender>,
}

// TODO: These should return results
impl<C: Connection> Transport<C> {
  pub fn new() -> Self {
    Self { inbound_connection: C::new(), outbound_connections: HashMap::new() }
  }

  pub fn send(&mut self, envelope: Envelope<C>, address: C::Address) {
    self.outbound_connections.get_mut(&address).map(|sender| sender.send(envelope));
  }

  pub fn broadcast(&mut self, envelope: Envelope<C>) {
    self.outbound_connections.values_mut().for_each(|sender| sender.send(envelope.clone()));
  }

  pub fn receive(&mut self) -> Option<Envelope<C>> { self.inbound_connection.receiver().receive() }

  pub fn add_outbound_connection(&mut self, address: C::Address, sender: C::Sender) {
    self.outbound_connections.insert(address, sender);
  }

  pub fn create_inbound_connection(&self) -> (C::Address, C::Sender) {
    self.inbound_connection.create_outbound_connection()
  }
}
