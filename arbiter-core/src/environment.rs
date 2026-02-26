pub trait Environment: Send + Sync + 'static {
  type State;
  type Update: Send + Sync + 'static;

  fn new() -> Self;

  fn get_state(&self) -> Self::State;

  fn update_state(&mut self, update: Self::Update);
}

impl Environment for () {
  type State = ();
  type Update = ();

  fn new() -> Self {}

  fn get_state(&self) -> Self::State {}

  fn update_state(&mut self, _update: Self::Update) {}
}
