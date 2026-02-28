use std::{
  any::{Any, TypeId},
  fmt::Debug,
  ops::Deref,
};

use crate::network::Network;

// The type that agents actually work with.
pub trait Message: Any + Send + Sync + Debug + 'static {}

// Blanket implementation for all types that meet the requirements
impl<T> Message for T where T: Send + Sync + Any + Debug + 'static {}

/// The envelope trait — each [`Network`] defines its own concrete envelope type.
pub trait Envelope: Clone + Send + Sync + Debug + 'static {
  fn type_id(&self) -> TypeId;
  fn wrap<M: Message>(message: M) -> Self;
  fn downcast<M: Message>(&self) -> Option<impl Deref<Target = M> + '_>;
}

pub trait Handler<M> {
  type Reply: Message;

  fn handle(&mut self, message: &M) -> Option<Self::Reply>;
}

#[allow(type_alias_bounds)]
pub type MessageHandlerFn<N: Network> =
  Box<dyn Fn(&mut dyn Any, &N::Envelope) -> Option<N::Envelope> + Send + Sync>;

pub fn create_handler<M, L, N>() -> MessageHandlerFn<N>
where
  L: Handler<M> + 'static,
  M: Message,
  N: Network,
{
  Box::new(move |agent: &mut dyn Any, envelope: &N::Envelope| {
    let Some(typed_agent) = agent.downcast_mut::<L>() else {
      unreachable!("type mismatch: agent is not the expected Handler type");
    };

    let Some(message) = envelope.downcast::<M>() else {
      tracing::error!(type_id = ?TypeId::of::<M>(), "failed to downcast message");
      return None;
    };

    typed_agent.handle(&*message).map(N::Envelope::wrap)
  })
}
