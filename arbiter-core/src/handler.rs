use std::{any::Any, ops::Deref, rc::Rc};

use serde::{Deserialize, Deserializer, Serializer};

use crate::transport::{Runtime, Transport};

/// Trait for types that can be sent as messages between agents
pub trait Message: Any + 'static {}

// Blanket implementation for all types that meet the requirements
impl<T> Message for T where T: Any + 'static {}

pub trait Handler<M> {
  type Reply: Message;

  fn handle(&mut self, message: &M) -> Self::Reply;
}

// TODO: I think we want this T::Payload to also have the ability to take a unit type ()
pub type MessageHandlerFn<T: Transport<R>, R: Runtime> =
  Box<dyn Fn(&mut dyn Any, T::Payload) -> T::Payload>;

// TODO: This panic is bad.
pub fn create_handler<'a, A, M, T, R>() -> MessageHandlerFn<T, R>
where
  A: Handler<M> + 'static,
  M: Message,
  T: Transport<R>,
  T::Payload: UnpackageMessage<M> + PackageMessage<A::Reply>,
  R: Runtime, {
  Box::new(|agent: &mut dyn Any, message: T::Payload| {
    if let Some(typed_agent) = agent.downcast_mut::<A>() {
      let message = message.unpackage().unwrap();
      let reply = typed_agent.handle(message);
      T::Payload::package(reply)
    } else {
      panic!("Agent does not handle message type");
    }
  })
}

pub trait UnpackageMessage<M: Message> {
  fn unpackage(&self) -> Option<&M>;
}

impl<M: Message> UnpackageMessage<M> for Rc<dyn Any> {
  fn unpackage(&self) -> Option<&M> { self.downcast_ref::<M>() }
}

// TODO: This is absolutely fucked. We need to find a better way to do this.
// impl<M: Message> UnpackageMessage<M> for Vec<u8>
// where M: for<'de> Deserialize<'de>
// {
//   fn unpackage(&self) -> Option<&M> {
//     Some(Box::leak(Box::new(serde_json::from_slice(self).unwrap())))
//   }
// }

pub trait PackageMessage<M: Message> {
  fn package(message: M) -> Self;
}

impl<M: 'static> PackageMessage<M> for Rc<M> {
  fn package(message: M) -> Self { Self::new(message) }
}

impl<T: 'static> PackageMessage<T> for Rc<dyn Any + 'static> {
  fn package(message: T) -> Self { Rc::new(message) }
}
