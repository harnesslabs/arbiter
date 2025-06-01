use std::{any::Any, sync::Arc};

/// Trait for types that can be sent as messages between agents
pub trait Message: Any + Send + Sync + 'static {}

// Blanket implementation for all types that meet the requirements
impl<T> Message for T where T: Any + Send + Sync + 'static {}

pub trait Handler<M> {
  type Reply;

  fn handle(&mut self, message: M) -> Self::Reply;
}

// Trait object for handling any message type
pub trait MessageHandler: Send + Sync {
  fn handle_message(&self, agent: &mut dyn Any, message: &dyn Any) -> Arc<dyn Any + Send + Sync>;
}

// Wrapper to make Handler<M> work as MessageHandler
pub(crate) struct HandlerWrapper<A, M> {
  pub(crate) _phantom: std::marker::PhantomData<(A, M)>,
}

impl<A, M> MessageHandler for HandlerWrapper<A, M>
where
  A: Handler<M> + Send + Sync + 'static,
  M: Message + Clone,
  <A as Handler<M>>::Reply: Send + Sync + 'static,
{
  fn handle_message(&self, agent: &mut dyn Any, message: &dyn Any) -> Arc<dyn Any + Send + Sync> {
    if let (Some(typed_agent), Some(typed_message)) =
      (agent.downcast_mut::<A>(), message.downcast_ref::<M>())
    {
      let reply = typed_agent.handle(typed_message.clone());
      Arc::new(reply)
    } else {
      // Return unit if types don't match
      Arc::new(())
    }
  }
}
