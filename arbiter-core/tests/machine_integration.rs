use arbiter_engine::{agent::Agent, world::World};

include!("common.rs");

#[derive(Debug, Deserialize, Serialize)]
struct MockBehavior;

impl Behavior<()> for MockBehavior {
  fn startup<S: StateDB>(
    &mut self,
    _client: Middleware<S>,
    _messager: Messager,
  ) -> impl std::future::Future<Output = Result<Option<EventStream<()>>, ArbiterEngineError>> + Send
  where
    S: Send + Sync,
    S::Location: Send + Sync,
    S::State: Send + Sync,
  {
    async move { Ok(None) }
  }
}

#[tokio::test]
async fn behavior_no_stream() {
  trace();
  let mut world = World::<String, String>::new("world");
  let behavior = MockBehavior;
  let agent = Agent::builder("agent").with_behavior(behavior);
  world.add_agent(agent);

  world.run().await.unwrap();
}
