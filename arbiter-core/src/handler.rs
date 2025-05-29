use std::{
  any::{Any, TypeId},
  marker::PhantomData,
};

use super::*;
use crate::agent::Context;

pub trait Handler<M> {
  type Reply;

  fn handle(&mut self, message: M) -> Self::Reply;
}

pub trait MessageHandler: Send + Sync {
  fn handle_message(&mut self, message: &dyn Any) -> Box<dyn Any>;
  fn message_type_id(&self) -> TypeId;
}

// Wrapper to make Handler<M> work as MessageHandler
pub(crate) struct HandlerWrapper<H, M> {
  pub(crate) handler:  H,
  pub(crate) _phantom: PhantomData<M>,
}

impl<H, M> MessageHandler for HandlerWrapper<H, M>
where
  H: Handler<M> + Send + Sync,
  M: Any + Clone + Send + Sync,
  H::Reply: Send + Sync + 'static,
{
  fn handle_message(&mut self, message: &dyn Any) -> Box<dyn Any> {
    if let Some(typed_message) = message.downcast_ref::<M>() {
      let reply = self.handler.handle(typed_message.clone());
      Box::new(reply)
    } else {
      // Return unit if message type doesn't match
      Box::new(())
    }
  }

  fn message_type_id(&self) -> TypeId { TypeId::of::<M>() }
}
