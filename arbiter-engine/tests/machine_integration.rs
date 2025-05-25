use arbiter_engine::{agent::Agent, world::World};

include!("common.rs");

#[derive(Debug, Deserialize, Serialize)]
struct MockBehavior;

#[async_trait::async_trait]
impl Behavior<()> for MockBehavior {
    async fn startup(
        &mut self,
        _client: Arc<ArbiterMiddleware>,
        _messager: Messager,
    ) -> Result<Option<EventStream<()>>> {
        Ok(None)
    }
}

#[tokio::test]
async fn behavior_no_stream() {
    trace();
    let mut world = World::new("test");
    let behavior = MockBehavior;
    let agent = Agent::builder("agent").with_behavior(behavior);
    world.add_agent(agent);

    world.run().await.unwrap();
}
