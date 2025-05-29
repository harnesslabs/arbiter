use super::*;

pub trait Handler<C> {
  type Message;
  type Reply;

  fn handle(&self, message: Self::Message, context: &mut C) -> Self::Reply;
}
