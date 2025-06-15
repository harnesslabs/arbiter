use std::{
  any::{Any, TypeId},
  fmt::Debug,
  ops::Deref,
  sync::Arc,
};

use serde::{Deserialize, Serialize};

use crate::connection::Transport;

// The type that agents actually work with.
pub trait Message: Any + Send + Sync + Debug + 'static {}

// Blanket implementation for all types that meet the requirements
impl<T> Message for T where T: Send + Sync + Any + Debug + 'static {}

// A version of th message that is sent "over the wire".
pub trait Payload: Clone + Send + Sync + Debug + 'static {}

impl Payload for Arc<dyn Message> {}

impl Payload for Vec<u8> {}

#[derive(Debug)]
pub struct Envelope<C: Transport> {
  pub payload: C::Payload,
  pub type_id: TypeId,
}

impl<C: Transport> Clone for Envelope<C> {
  fn clone(&self) -> Self { Self { payload: self.payload.clone(), type_id: self.type_id } }
}

impl<C: Transport> Envelope<C> {
  pub fn package<M: Message>(message: M) -> Self
  where C::Payload: Package<M> {
    Self { payload: C::Payload::package(message), type_id: TypeId::of::<M>() }
  }

  pub fn unpackage<M: Message>(&self) -> Option<impl Deref<Target = M> + '_>
  where C::Payload: Unpacackage<M> {
    self.payload.unpackage()
  }
}

pub trait Package<M: Message> {
  fn package(message: M) -> Self;
}

impl<T: Message> Package<T> for Arc<dyn Message> {
  fn package(message: T) -> Self { Arc::new(message) }
}

impl<M> Package<M> for Vec<u8>
where M: Message + Serialize
{
  fn package(message: M) -> Self { serde_json::to_vec(&message).unwrap() }
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
where M: Message + for<'de> Deserialize<'de>
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
  fn from(message: M) -> Self { Self::Message(message) }
}

impl<M: Message> From<Option<M>> for HandleResult<M> {
  fn from(message: Option<M>) -> Self { message.map_or(Self::None, Self::Message) }
}

pub trait Handler<M> {
  type Reply: Message;

  fn handle(&mut self, message: &M) -> impl Into<HandleResult<Self::Reply>>;
}

pub type MessageHandlerFn<C: Transport> =
  Box<dyn Fn(&mut dyn Any, C::Payload) -> HandleResult<C::Payload> + Send + Sync>;

// TODO: This panic is bad.
pub fn create_handler<M, L, C>() -> MessageHandlerFn<C>
where
  L: Handler<M> + 'static,
  M: Message,
  C: Transport,
  C::Payload: Unpacackage<M> + Package<L::Reply>, {
  Box::new(|agent: &mut dyn Any, message_payload: C::Payload| {
    agent.downcast_mut::<L>().map_or_else(
      || {
        unreachable!(
          "This should never happen as we've already checked the `Agent` type from the call site"
        );
      },
      |typed_agent| {
        let unpacked_message_option = message_payload.unpackage();
        unpacked_message_option.map_or_else(
          || panic!("Failed to unpackage message of type {:?}", std::any::TypeId::of::<M>()),
          |unpacked_message| {
            let reply = typed_agent.handle(&*unpacked_message).into();
            println!("reply for agent is {:?}", reply);
            match reply {
              HandleResult::Message(message) => {
                println!("reply for agent is {:?}", message);
                HandleResult::Message(C::Payload::package(message))
              },
              HandleResult::None => HandleResult::None,
              HandleResult::Stop => HandleResult::Stop,
            }
          },
        )
      },
    )
  })
}
