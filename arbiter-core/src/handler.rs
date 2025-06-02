use std::{any::Any, rc::Rc};

/// Trait for types that can be sent as messages between agents
pub trait Message: Any + 'static {}

// Blanket implementation for all types that meet the requirements
impl<T> Message for T where T: Any + 'static {}

pub trait Handler<M> {
  type Reply: Message;

  fn handle(&mut self, message: &M) -> Self::Reply;
}

pub type MessageHandlerFn = Box<dyn Fn(&mut dyn Any, &dyn Any) -> Rc<dyn Any>>;

pub fn create_handler<A, M>() -> MessageHandlerFn
where
  A: Handler<M> + 'static,
  M: Message, {
  Box::new(|agent: &mut dyn Any, message: &dyn Any| {
    if let (Some(typed_agent), Some(typed_message)) =
      (agent.downcast_mut::<A>(), message.downcast_ref::<M>())
    {
      let reply = typed_agent.handle(typed_message);
      Rc::new(reply)
    } else {
      Rc::new(())
    }
  })
}
