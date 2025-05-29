use super::*;
use crate::agent::Context;

pub trait Handler<A> {
  type Message;
  type Reply;

  fn handle(&self, message: Self::Message, agent: &mut A) -> Self::Reply;
}
