use crate::{
  agent::LifeCycle,
  handler::{Handler, Message},
};

pub trait Environment: LifeCycle + Handler<Self::Instruction> {
  type Instruction: Message;

  fn new() -> Self;
}

impl LifeCycle for () {
  type Snapshot = ();
  type StartMessage = ();
  type StopMessage = ();
  fn on_start(&mut self) -> Self::StartMessage {}
  fn on_stop(&mut self) -> Self::StopMessage {}
  fn snapshot(&self) -> Self::Snapshot {}
}

impl Handler<()> for () {
  type Reply = ();

  fn handle(&mut self, _message: &()) {}
}

impl Environment for () {
  type Instruction = ();

  fn new() -> Self {}
}
