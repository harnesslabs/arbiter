use std::{
  any::{Any, TypeId},
  fmt::Debug,
  ops::Deref,
};

use crate::network::{Network, Socket};

/// The base trait for all messages processed by actors.
///
/// This trait is automatically implemented for any type that satisfies
/// its bounds. When the `tcp` feature is enabled, messages must also be
/// serializable via `serde`.
#[cfg(feature = "tcp")]
pub trait Message:
  Any + Send + Sync + Debug + serde::Serialize + serde::de::DeserializeOwned + 'static {
}

#[cfg(not(feature = "tcp"))]
pub trait Message: Any + Send + Sync + Debug + 'static {}

#[cfg(feature = "tcp")]
impl<T> Message for T where T: Send + Sync + Any + Debug + serde::Serialize + serde::de::DeserializeOwned + 'static {}

#[cfg(not(feature = "tcp"))]
impl<T> Message for T where T: Send + Sync + Any + Debug + 'static {}

/// The envelope trait — each [`Network`] defines its own concrete envelope type
/// which allows it to transport arbitrary [`Message`] payloads.
pub trait Envelope: Send + Sync + Debug + 'static {
  /// Returns the `TypeId` of the underlying message.
  fn type_id(&self) -> TypeId;
  /// Wraps a strongly-typed [`Message`] into this envelope.
  fn wrap<M: Message>(message: M) -> Self;
  /// Attempts to downcast the envelope payload back to a specific [`Message`] type.
  fn downcast<M: Message>(&self) -> Option<impl Deref<Target = M> + '_>;
  /// Registers the type internally if the network backend requires it.
  fn register_type<M: Message>() {}
}

/// Defines how an actor processes a specific type of [`Message`].
pub trait Handler<M> {
  /// The type of message to reply with. Use `()` if no reply is needed.
  type Reply: Message;

  /// Handles the incoming message, optionally returning a reply.
  fn handle(&mut self, message: &M) -> Option<Self::Reply>;
}

/// A type-erased handler function that wraps a specific `Handler::handle` implementation.
#[allow(type_alias_bounds)]
pub(crate) type MessageHandlerFn<N: Network> = Box<
  dyn Fn(&mut dyn Any, &<N::Socket as Socket>::Envelope) -> Option<<N::Socket as Socket>::Envelope>
    + Send
    + Sync,
>;

/// Creates a type-erased message handler function from a concrete `Handler` implementation.
///
/// This allows the actor to store a heterogeneous collection of handlers
/// and dispatch messages to them dynamically based on `TypeId`.
pub(crate) fn create_handler<M, L, N>() -> MessageHandlerFn<N>
where
  L: Handler<M> + 'static,
  M: Message,
  N: Network, {
  Box::new(move |agent: &mut dyn Any, envelope: &<N::Socket as Socket>::Envelope| {
    let Some(typed_agent) = agent.downcast_mut::<L>() else {
      unreachable!("type mismatch: agent is not the expected Handler type");
    };

    let Some(message) = envelope.downcast::<M>() else {
      tracing::error!(type_id = ?TypeId::of::<M>(), "failed to downcast message");
      return None;
    };

    typed_agent.handle(&*message).map(<N::Socket as Socket>::Envelope::wrap)
  })
}
