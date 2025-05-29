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

// Trait object for handling any message type
pub trait MessageHandler: Send + Sync {
  fn handle_message(&mut self, message: &dyn Any) -> Box<dyn Any + Send + Sync>;
}

// Wrapper to make Handler<M> work as MessageHandler
pub struct HandlerWrapper<H, M> {
  pub handler:  H,
  pub _phantom: PhantomData<M>,
}

impl<H, M> MessageHandler for HandlerWrapper<H, M>
where
  H: Handler<M> + Send + Sync,
  M: Any + Clone + Send + Sync,
  H::Reply: Send + Sync + 'static,
{
  fn handle_message(&mut self, message: &dyn Any) -> Box<dyn Any + Send + Sync> {
    if let Some(typed_message) = message.downcast_ref::<M>() {
      let reply = self.handler.handle(typed_message.clone());
      Box::new(reply)
    } else {
      // Return unit if message type doesn't match
      Box::new(())
    }
  }
}
