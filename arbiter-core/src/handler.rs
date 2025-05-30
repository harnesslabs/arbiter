use std::{any::Any, marker::PhantomData};

pub trait Handler<M> {
  type Reply;

  fn handle(&mut self, message: M) -> Self::Reply;
}

// Trait object for handling any message type
pub trait MessageHandler: Send + Sync {
  fn handle_message(&mut self, message: &dyn Any) -> Box<dyn Any + Send + Sync>;
}

// Wrapper to make Handler<M> work as MessageHandler
pub struct HandlerWrapper<A, H: Handler<M>, M> {
  pub handler:  fn(&mut A, M) -> H::Reply,
  pub _phantom: PhantomData<M>,
}

impl<A, H: Handler<M>, M> HandlerWrapper<A, H, M> {
  pub fn new(handler: fn(&mut A, M) -> H::Reply) -> Self { Self { handler, _phantom: PhantomData } }
}

impl<A, H: Handler<M>, M> MessageHandler for HandlerWrapper<A, H, M>
where
  H: Handler<M>,
  M: Any + Clone + Send + Sync,
  H::Reply: Send + Sync + 'static,
{
  fn handle_message(&mut self, message: &dyn Any) -> Box<dyn Any + Send + Sync> {
    if let Some(typed_message) = message.downcast_ref::<M>() {
      let reply = (self.handler)(typed_message.clone());
      Box::new(reply)
    } else {
      // Return unit if message type doesn't match
      Box::new(())
    }
  }
}
