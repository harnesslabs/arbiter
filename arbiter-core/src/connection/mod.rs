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

pub trait Connection: Send + Sync + 'static {
  type Address: Copy + Send + Sync + PartialEq + Eq + Hash + std::fmt::Debug + std::fmt::Display;
  type Payload: Clone + Message + Package<Self::Payload>;
  type Sender;
  type Receiver;

  fn new() -> Self;

  fn address(&self) -> Self::Address;

  fn get_sender(&self) -> Self::Sender;

  fn send(&mut self, envelope: Envelope<Self>)
  where Self: Sized;

  fn receive(&mut self) -> Option<Envelope<Self>>
  where Self: Sized;
}

pub struct Transport<C: Connection> {
  pub(crate) inbound_connection:   C,
  pub(crate) outbound_connections: HashMap<C::Address, C>,
}

// TODO: These should return results
impl<C: Connection> Transport<C> {
  pub fn new() -> Self {
    Self { inbound_connection: C::new(), outbound_connections: HashMap::new() }
  }

  pub fn send(&mut self, envelope: Envelope<C>, address: C::Address) {
    self.outbound_connections.get_mut(&address).map(|connection| connection.send(envelope));
  }

  pub fn broadcast(&mut self, envelope: Envelope<C>) {
    self.outbound_connections.values_mut().for_each(|connection| connection.send(envelope.clone()));
  }

  pub fn receive(&mut self) -> Option<Envelope<C>> { self.inbound_connection.receive() }

  pub fn add_outbound_connection(&mut self, connection: C) {
    self.outbound_connections.insert(connection.address(), connection);
  }
}
