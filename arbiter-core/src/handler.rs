use super::*;
use crate::agent::Context;

pub trait Handler<M> {
  type Reply;

  fn handle(&mut self, message: M) -> Self::Reply;
}
