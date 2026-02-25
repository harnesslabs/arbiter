use std::{
  any::{Any, TypeId},
  fmt::Debug,
  ops::Deref,
  sync::Arc,
};

use serde::{Deserialize, Serialize};

use crate::{
  network::Network,
  protocol::{AgentId, CorrelationId, EnvelopeMeta, MessageKind, SchemaVersion},
};

// The type that agents actually work with.
pub trait Message: Any + Send + Sync + Debug + 'static {}

// Blanket implementation for all types that meet the requirements
impl<T> Message for T where T: Send + Sync + Any + Debug + 'static {}

// A version of th message that is sent "over the wire".
pub trait Payload: Clone + Send + Sync + Debug + 'static {}

impl Payload for Arc<dyn Message> {}

impl Payload for Vec<u8> {}

pub struct Envelope<N: Network> {
  pub payload: N::Payload,
  pub meta: EnvelopeMeta,
  pub type_id: TypeId,
}

impl<N: Network> Debug for Envelope<N> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(
      f,
      "Envelope {{ payload: {:?}, meta: {:?}, type_id: {:?} }}",
      self.payload, self.meta, self.type_id
    )
  }
}

impl<N: Network> Clone for Envelope<N> {
  fn clone(&self) -> Self {
    Self { payload: self.payload.clone(), meta: self.meta.clone(), type_id: self.type_id }
  }
}

impl<N: Network> Envelope<N> {
  pub fn package<M: Message>(message: M) -> Self
  where
    N::Payload: Package<M>,
  {
    Self {
      payload: N::Payload::package(message),
      meta: EnvelopeMeta::for_type::<M>(),
      type_id: TypeId::of::<M>(),
    }
  }

  pub fn package_with_meta<M: Message>(message: M, meta: EnvelopeMeta) -> Self
  where
    N::Payload: Package<M>,
  {
    Self { payload: N::Payload::package(message), meta, type_id: TypeId::of::<M>() }
  }

  pub fn unpackage<M: Message>(&self) -> Option<impl Deref<Target = M> + '_>
  where
    N::Payload: Unpacackage<M>,
  {
    self.payload.unpackage()
  }

  pub fn with_meta(mut self, meta: EnvelopeMeta) -> Self {
    self.meta = meta;
    self
  }

  pub fn with_sender(mut self, sender: impl Into<AgentId>) -> Self {
    self.meta = self.meta.with_sender(sender);
    self
  }

  pub fn with_sender_address<A: std::fmt::Display>(self, address: A) -> Self {
    self.with_sender(address.to_string())
  }

  pub fn with_correlation_id(mut self, correlation_id: CorrelationId) -> Self {
    self.meta = self.meta.with_correlation_id(correlation_id);
    self
  }

  pub fn with_schema_version(mut self, schema_version: SchemaVersion) -> Self {
    self.meta = self.meta.with_schema_version(schema_version);
    self
  }

  pub fn to_agent(mut self, agent_id: impl Into<AgentId>) -> Self {
    self.meta = self.meta.to_agent(agent_id);
    self
  }

  pub fn to_address<A: std::fmt::Display>(self, address: A) -> Self {
    self.to_agent(address.to_string())
  }

  pub fn to_group(mut self, group: impl Into<String>) -> Self {
    self.meta = self.meta.to_group(group);
    self
  }

  pub fn broadcast(mut self) -> Self {
    self.meta = self.meta.broadcast();
    self
  }
}

pub trait Package<M: Message> {
  fn package(message: M) -> Self;
}

impl<M: Message> Package<M> for Arc<dyn Message> {
  fn package(message: M) -> Self {
    Arc::new(message)
  }
}

impl<M> Package<M> for Vec<u8>
where
  M: Message + Serialize,
{
  fn package(message: M) -> Self {
    serde_json::to_vec(&message).unwrap()
  }
}

pub trait Unpacackage<M: Message> {
  fn unpackage(&self) -> Option<impl Deref<Target = M>>;
}

impl<M: Message> Unpacackage<M> for Arc<dyn Message> {
  fn unpackage(&self) -> Option<impl Deref<Target = M>> {
    (self.as_ref() as &dyn Any).downcast_ref::<M>()
  }
}

impl<M> Unpacackage<M> for Vec<u8>
where
  M: Message + for<'de> Deserialize<'de>,
{
  fn unpackage(&self) -> Option<impl Deref<Target = M>> {
    serde_json::from_slice(self).ok().map(Box::new)
  }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum HandleResult<M: Message> {
  Message(M),
  None,
  Stop,
}

impl<M: Message> From<M> for HandleResult<M> {
  fn from(message: M) -> Self {
    Self::Message(message)
  }
}

impl<M: Message> From<Option<M>> for HandleResult<M> {
  fn from(message: Option<M>) -> Self {
    message.map_or(Self::None, Self::Message)
  }
}

pub trait Handler<M> {
  type Reply: Message;

  #[allow(refining_impl_trait)]
  fn handle(&mut self, message: &M) -> impl Into<HandleResult<Self::Reply>>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HandlerError {
  AgentTypeMismatch {
    expected: &'static str,
  },
  PayloadDecodeFailed {
    message_kind: MessageKind,
    schema_version: SchemaVersion,
    expected: &'static str,
  },
}

impl std::fmt::Display for HandlerError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::AgentTypeMismatch { expected } => {
        write!(f, "agent type mismatch while dispatching handler for {expected}")
      },
      Self::PayloadDecodeFailed { message_kind, schema_version, expected } => write!(
        f,
        "failed to decode payload for message kind '{}' v{} as {}",
        message_kind, schema_version, expected
      ),
    }
  }
}

impl std::error::Error for HandlerError {}

pub type HandlerDispatchResult<C> = Result<HandleResult<Envelope<C>>, HandlerError>;

#[allow(type_alias_bounds)]
pub type MessageHandlerFn<C: Network> =
  Box<dyn Fn(&mut dyn Any, Envelope<C>) -> HandlerDispatchResult<C> + Send + Sync>;

pub fn create_handler<M, L, N>() -> MessageHandlerFn<N>
where
  L: Handler<M> + 'static,
  M: Message,
  N: Network,
  N::Payload: Unpacackage<M> + Package<L::Reply>,
{
  Box::new(|agent: &mut dyn Any, envelope: Envelope<N>| {
    let typed_agent = agent
      .downcast_mut::<L>()
      .ok_or(HandlerError::AgentTypeMismatch { expected: std::any::type_name::<L>() })?;

    let Envelope { payload, meta, .. } = envelope;

    let unpacked_message =
      payload.unpackage().ok_or_else(|| HandlerError::PayloadDecodeFailed {
        message_kind: meta.message_kind.clone(),
        schema_version: meta.schema_version,
        expected: std::any::type_name::<M>(),
      })?;

    let reply = typed_agent.handle(&*unpacked_message).into();
    Ok(match reply {
      HandleResult::Message(message) => {
        let reply_envelope =
          Envelope::package(message).with_correlation_id(CorrelationId::from(meta.message_id));
        HandleResult::Message(reply_envelope)
      },
      HandleResult::None => HandleResult::None,
      HandleResult::Stop => HandleResult::Stop,
    })
  })
}

#[cfg(test)]
mod tests {
  use crate::{fixtures::*, network::memory::InMemory};

  use super::*;

  #[test]
  fn create_handler_returns_structured_decode_error() {
    let mut logger = Logger { name: "logger".to_string(), message_count: 0 };
    let handler = create_handler::<TextMessage, Logger, InMemory>();
    let envelope = Envelope::<InMemory>::package(NumberMessage { value: 7 });

    let error = handler(&mut logger, envelope).expect_err("expected decode error");

    match error {
      HandlerError::PayloadDecodeFailed { message_kind, schema_version, expected } => {
        assert_eq!(message_kind, MessageKind::for_type::<NumberMessage>());
        assert_eq!(schema_version, SchemaVersion(1));
        assert_eq!(expected, std::any::type_name::<TextMessage>());
      },
      other => panic!("unexpected error: {other:?}"),
    }
  }
}
