use std::collections::HashMap;

use arbiter_core::{agent::Agent, universe::Universe, world::World};
use tracing_test::traced_test;

use super::*;

const AGENT_ID: &str = "agent";

#[tokio::test]
#[traced_test]
async fn behavior_no_stream() {
  let mut world = World::<()>::new("world");
  let behavior = MockBehavior;
  let agent = Agent::builder("agent").with_behavior(behavior);
  world.add_agent(agent);

  let _world = world.run().await.unwrap();
}

#[tokio::test]
#[traced_test]
async fn echoer() {
  let mut world = World::<HashMap<String, String>>::new("world");

  let agent = Agent::builder(AGENT_ID);
  let behavior = TimedMessage::new(
    1,
    "Hello, world!".to_owned(),
    "Hello, world!".to_owned(),
    Some(2),
    Some("Hello, world!".to_owned()),
  );
  world.add_agent(agent.with_behavior(behavior));

  let _world = world.run().await.unwrap();

  // Check that the expected messages were logged
  logs_contain("Hello, world!");
  logs_contain("Reached max count (2), halting behavior");
}

#[tokio::test]
#[traced_test]
async fn ping_pong() {
  let mut world = World::<HashMap<String, String>>::new("world");

  let agent = Agent::builder(AGENT_ID);
  let behavior_ping =
    TimedMessage::new(1, "pong".to_owned(), "ping".to_owned(), Some(2), Some("ping".to_owned()));
  let behavior_pong = TimedMessage::new(1, "ping".to_owned(), "pong".to_owned(), Some(2), None);

  world.add_agent(agent.with_behavior(behavior_ping).with_behavior(behavior_pong));

  let _world = world.run().await.unwrap();

  // Check that ping and pong messages were logged
  logs_contain("ping");
  logs_contain("pong");
  logs_contain("Reached max count (2), halting behavior");
}

#[tokio::test]
#[traced_test]
async fn ping_pong_two_agent() {
  let mut world = World::<HashMap<String, String>>::new("world");

  let agent_ping = Agent::builder("agent_ping");
  let agent_pong = Agent::builder("agent_pong");

  let behavior_ping =
    TimedMessage::new(1, "pong".to_owned(), "ping".to_owned(), Some(2), Some("ping".to_owned()));
  let behavior_pong = TimedMessage::new(1, "ping".to_owned(), "pong".to_owned(), Some(2), None);

  world.add_agent(agent_ping.with_behavior(behavior_ping));
  world.add_agent(agent_pong.with_behavior(behavior_pong));

  let _world = world.run().await.unwrap();

  // Check that ping and pong messages were logged from both agents
  logs_contain("ping");
  logs_contain("pong");
  logs_contain("Reached max count (2), halting behavior");
}

#[tokio::test]
async fn config_test() {
  let world =
    World::<HashMap<String, String>>::from_config::<Behaviors>("tests/config.toml").unwrap();
  assert_eq!(world.id, "timed_message_world");
  let _world = world.run().await.unwrap();
}

#[tokio::test]
#[traced_test]
async fn run_parallel() {
  let mut world1 = World::<HashMap<String, String>>::new("test1");
  let agent1 = Agent::builder("agent1");
  let behavior1 =
    TimedMessage::new(1, "echo".to_owned(), "echo".to_owned(), Some(5), Some("echo".to_owned()));
  world1.add_agent(agent1.with_behavior(behavior1));

  let mut world2 = World::<HashMap<String, String>>::new("test2");
  let agent2 = Agent::builder("agent2");
  let behavior2 =
    TimedMessage::new(1, "echo".to_owned(), "echo".to_owned(), Some(5), Some("echo".to_owned()));
  world2.add_agent(agent2.with_behavior(behavior2));

  let mut universe = Universe::new();
  universe.add_world(world1);
  universe.add_world(world2);

  let _universe = universe.run_worlds().await.unwrap();

  // With tracing-test, we can check the logs using logs_contain
  logs_contain("Engaging behavior Some(\"agent1\")");
  logs_contain("Engaging behavior Some(\"agent2\")");
}
