const AGENT_ID: &str = "agent";

use std::time::Duration;

use arbiter_engine::{
    agent::Agent,
    machine::{CreateStateMachine, Engine, StateMachine},
    world::World,
};
use arbiter_macros::Behaviors;
use futures_util::StreamExt;
use tokio::time::timeout;
include!("common.rs");

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn echoer() {
    let mut world = World::new("world");

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
            }
            _ => {
                panic!("Timeout reached. Test failed.");
            }
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn ping_pong() {
    let mut world = World::new("world");

    let agent = Agent::builder(AGENT_ID);
    let behavior_ping = TimedMessage::new(
        1,
        "pong".to_owned(),
        "ping".to_owned(),
        Some(2),
        Some("ping".to_owned()),
    );
    let behavior_pong = TimedMessage::new(1, "ping".to_owned(), "pong".to_owned(), Some(2), None);

    world.add_agent(
        agent
            .with_behavior(behavior_ping)
            .with_behavior(behavior_pong),
    );

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
            }
            _ => {
                panic!("Timeout reached. Test failed.");
            }
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn ping_pong_two_agent() {
    let mut world = World::new("world");

    let agent_ping = Agent::builder("agent_ping");
    let agent_pong = Agent::builder("agent_pong");

    let behavior_ping = TimedMessage::new(
        1,
        "pong".to_owned(),
        "ping".to_owned(),
        Some(2),
        Some("ping".to_owned()),
    );
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
            }
            _ => {
                panic!("Timeout reached. Test failed.");
            }
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Behaviors)]
enum Behaviors {
    TimedMessage(TimedMessage),
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn config_test() {
    let mut world = World::from_config::<Behaviors>("tests/config.toml").unwrap();
    assert_eq!(world.id, "timed_message_world");
    world.run().await.unwrap();
}
