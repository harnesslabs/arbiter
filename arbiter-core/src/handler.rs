use std::{any::Any, ops::Deref, rc::Rc, sync::Arc};

use serde::Deserialize;

use crate::transport::Transport;

/// Trait for types that can be sent as messages between agents
pub trait Message: Send + Sync + Any + 'static {}

// Blanket implementation for all types that meet the requirements
impl<T> Message for T where T: Send + Sync + Any + 'static {}

pub trait Handler<M> {
  type Reply: Message;

  fn handle(&mut self, message: &M) -> Self::Reply;
}

// TODO: I think we want this T::Payload to also have the ability to take a unit type ()
pub type MessageHandlerFn<T: Transport> =
  Box<dyn Fn(&mut dyn Any, T::Payload) -> T::Payload + Send + Sync>;

// TODO: This panic is bad.
pub fn create_handler<'a_unused, M, L, T>() -> MessageHandlerFn<T>
where
  L: Handler<M> + 'static,
  M: Message,
  T: Transport,
  T::Payload: UnpackageMessage<M> + PackageMessage<L::Reply>, {
  Box::new(|agent: &mut dyn Any, message_payload: T::Payload| {
    agent.downcast_mut::<L>().map_or_else(
      || {
        unreachable!(
          "This should never happen as we've already checked the `Agent` type from the call site"
        );
      },
      |typed_agent| {
        let unpacked_message_option = message_payload.unpackage();
        match unpacked_message_option {
          Some(unpacked_message) => {
            let reply = typed_agent.handle(&*unpacked_message);
            T::Payload::package(reply)
          },
          None => panic!("Failed to unpackage message of type {:?}", std::any::TypeId::of::<M>()),
        }
      },
    )
  })
}

pub trait UnpackageMessage<M: Message> {
  fn unpackage(&self) -> Option<impl Deref<Target = M>>;
}

impl<M: Message> UnpackageMessage<M> for Arc<dyn Any> {
  fn unpackage(&self) -> Option<impl Deref<Target = M>> { self.downcast_ref::<M>() }
}

impl<M> UnpackageMessage<M> for Vec<u8>
where M: Message + for<'de> Deserialize<'de>
{
  fn unpackage(&self) -> Option<impl Deref<Target = M>> {
    serde_json::from_slice(self).ok().map(Box::new)
  }
}

pub trait PackageMessage<M: Message> {
  fn package(message: M) -> Self;
}

impl<M: Message> PackageMessage<M> for Arc<M> {
  fn package(message: M) -> Self { Arc::new(message) }
}

impl<T: Message> PackageMessage<T> for Arc<dyn Message> {
  fn package(message: T) -> Self { Arc::new(message) }
}
