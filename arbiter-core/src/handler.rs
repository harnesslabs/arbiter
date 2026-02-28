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
}

pub trait Package<M: Message> {
  fn package(message: M) -> Self;
}

pub trait Unpackage<M: Message> {
  fn unpackage(&self) -> Option<impl Deref<Target = M>>;
}

#[derive(Debug)]
pub enum HandleResult<M: Message> {
  Message(M),
  None,
  Stop,
}

impl<M: Message> HandleResult<M> {
  pub fn map<R: Message>(self, f: impl FnOnce(M) -> R) -> HandleResult<R> {
    match self {
      Self::Message(m) => HandleResult::Message(f(m)),
      Self::None => HandleResult::None,
      Self::Stop => HandleResult::Stop,
    }
  }
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

  fn handle(&mut self, message: &M) -> impl Into<HandleResult<Self::Reply>>;
}

#[allow(type_alias_bounds)]
pub type MessageHandlerFn<N: Network> =
  Box<dyn Fn(&mut dyn Any, &N::Envelope) -> HandleResult<N::Envelope> + Send + Sync>;

pub fn create_handler<M, L, N>() -> MessageHandlerFn<N>
where
  L: Handler<M> + 'static,
  M: Message,
  N: Network,
  N::Envelope: Unpackage<M> + Package<L::Reply>,
{
  Box::new(move |agent: &mut dyn Any, envelope: &N::Envelope| {
    let Some(typed_agent) = agent.downcast_mut::<L>() else {
      unreachable!("type mismatch: agent is not the expected Handler type");
    };

    let Some(message) = envelope.unpackage() else {
      tracing::error!(type_id = ?TypeId::of::<M>(), "failed to unpackage message");
      return HandleResult::None;
    };

    typed_agent.handle(&*message).into().map(Package::package)
  })
}
