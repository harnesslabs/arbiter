pub trait Environment {
  type State;
  type Update;

  fn new() -> Self;

  fn get_state(&self) -> Self::State;

  fn update_state(&mut self, update: Self::Update);
}
