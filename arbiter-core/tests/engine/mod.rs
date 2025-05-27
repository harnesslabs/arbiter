use std::{collections::HashMap, fs::read_to_string, time::Duration};

use arbiter_core::{agent::Agent, universe::Universe, world::World};
use futures::StreamExt;
use tokio::time::timeout;
use tracing_subscriber::{fmt, EnvFilter};

use super::*;

const AGENT_ID: &str = "agent";

#[tokio::test]
async fn behavior_no_stream() {
  trace();
  let mut world = World::<()>::new("world");
  let behavior = MockBehavior;
  let agent = Agent::builder("agent").with_behavior(behavior);
  world.add_agent(agent);

  world.run().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
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
  let messager = world.messager.for_agent("outside_world");

  world.run().await.unwrap();

  let mut stream = messager.stream().unwrap();
  let mut idx = 0;

  loop {
    match timeout(Duration::from_secs(1), stream.next()).await {
      Ok(Some(event)) => {
        println!("Event received in outside world: {:?}", event);
        idx += 1;
        if idx == 2 {
          break;
        }
      },
      _ => {
        panic!("Timeout reached. Test failed.");
      },
    }
  }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn ping_pong() {
  let mut world = World::<HashMap<String, String>>::new("world");

  let agent = Agent::builder(AGENT_ID);
  let behavior_ping =
    TimedMessage::new(1, "pong".to_owned(), "ping".to_owned(), Some(2), Some("ping".to_owned()));
  let behavior_pong = TimedMessage::new(1, "ping".to_owned(), "pong".to_owned(), Some(2), None);

  world.add_agent(agent.with_behavior(behavior_ping).with_behavior(behavior_pong));

  let messager = world.messager.for_agent("outside_world");
  world.run().await.unwrap();

  let mut stream = messager.stream().unwrap();
  let mut idx = 0;

  loop {
    match timeout(Duration::from_secs(1), stream.next()).await {
      Ok(Some(event)) => {
        println!("Event received in outside world: {:?}", event);
        idx += 1;
        if idx == 4 {
          break;
        }
      },
      _ => {
        panic!("Timeout reached. Test failed.");
      },
    }
  }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn ping_pong_two_agent() {
  let mut world = World::<HashMap<String, String>>::new("world");

  let agent_ping = Agent::builder("agent_ping");
  let agent_pong = Agent::builder("agent_pong");

  let behavior_ping =
    TimedMessage::new(1, "pong".to_owned(), "ping".to_owned(), Some(2), Some("ping".to_owned()));
  let behavior_pong = TimedMessage::new(1, "ping".to_owned(), "pong".to_owned(), Some(2), None);

  world.add_agent(agent_ping.with_behavior(behavior_ping));
  world.add_agent(agent_pong.with_behavior(behavior_pong));

  let messager = world.messager.for_agent("outside_world");
  world.run().await.unwrap();

  let mut stream = messager.stream().unwrap();
  let mut idx = 0;

  loop {
    match timeout(Duration::from_secs(1), stream.next()).await {
      Ok(Some(event)) => {
        println!("Event received in outside world: {:?}", event);
        idx += 1;
        if idx == 5 {
          break;
        }
      },
      _ => {
        panic!("Timeout reached. Test failed.");
      },
    }
  }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn config_test() {
  let mut world =
    World::<HashMap<String, String>>::from_config::<Behaviors>("tests/config.toml").unwrap();
  assert_eq!(world.id, "timed_message_world");
  world.run().await.unwrap();
}

#[tokio::test]
async fn run_parallel() {
  std::env::set_var("RUST_LOG", "trace");

  let temp_file = tempfile::NamedTempFile::new().expect("Unable to create temp file");
  let temp_path = temp_file.path().to_owned();

  let file = temp_file.reopen().expect("Unable to reopen temp file");

  let subscriber = fmt().with_env_filter(EnvFilter::from_default_env()).with_writer(file).finish();

  tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

  let mut world1 = World::<HashMap<String, String>>::new("test1");
  let agent1 = Agent::builder("agent1");
  let behavior1 =
    TimedMessage::new(1, "echo".to_owned(), "echo".to_owned(), Some(5), Some("echo".to_owned()));
  world1.add_agent(agent1.with_behavior(behavior1));

  let mut world2 = World::new("test2");
  let agent2 = Agent::builder("agent2");
  let behavior2 =
    TimedMessage::new(1, "echo".to_owned(), "echo".to_owned(), Some(5), Some("echo".to_owned()));
  world2.add_agent(agent2.with_behavior(behavior2));

  let mut universe = Universe::new();
  universe.add_world(world1);
  universe.add_world(world2);

  universe.run_worlds().await.unwrap();

  // Read the log file contents
  let parsed_file = read_to_string(&temp_path).expect("Unable to read log file");

  // Define the line to check (excluding the timestamp)
  let line_to_check = "Engaging behavior";

  // Assert that both agents have their engaging behavior logs
  let agent1_found = parsed_file.contains("Engaging behavior Some(\"agent1\")");
  let agent2_found = parsed_file.contains("Engaging behavior Some(\"agent2\")");

  assert!(agent1_found, "Agent1 engaging behavior log not found");
  assert!(agent2_found, "Agent2 engaging behavior log not found");

  // Also check that we have exactly 2 "Engaging behavior" lines
  let engaging_count = parsed_file.matches("Engaging behavior").count();
  assert_eq!(
    engaging_count, 2,
    "Expected exactly 2 'Engaging behavior' logs, found {}",
    engaging_count
  );
}
