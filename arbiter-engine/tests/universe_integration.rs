use std::fs::{read_to_string, remove_file, File};

use arbiter_engine::{agent::Agent, universe::Universe, world::World};
use tracing_subscriber::{fmt, EnvFilter};
include!("common.rs");

#[tokio::test]
async fn run_parallel() {
    std::env::set_var("RUST_LOG", "trace");
    let file = File::create("test_logs_engine.log").expect("Unable to create log file");

    let subscriber = fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(file)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    let mut world1 = World::new("test1");
    let agent1 = Agent::builder("agent1");
    let behavior1 = TimedMessage::new(
        1,
        "echo".to_owned(),
        "echo".to_owned(),
        Some(5),
        Some("echo".to_owned()),
    );
    world1.add_agent(agent1.with_behavior(behavior1));

    let mut world2 = World::new("test2");
    let agent2 = Agent::builder("agent2");
    let behavior2 = TimedMessage::new(
        1,
        "echo".to_owned(),
        "echo".to_owned(),
        Some(5),
        Some("echo".to_owned()),
    );
    world2.add_agent(agent2.with_behavior(behavior2));

    let mut universe = Universe::new();
    universe.add_world(world1);
    universe.add_world(world2);

    universe.run_worlds().await.unwrap();

    let parsed_file = read_to_string("test_logs_engine.log").expect("Unable to read log file");

    // Define the line to check (excluding the timestamp)
    let line_to_check = "Behavior is starting up.";

    // Assert that the lines appear consecutively
    assert!(
        lines_appear_consecutively(&parsed_file, line_to_check),
        "The lines do not appear consecutively"
    );
    remove_file("test_logs_engine.log").expect(
        "Unable to remove log
        file",
    );
}

fn lines_appear_consecutively(file_contents: &str, line_to_check: &str) -> bool {
    let mut lines = file_contents.lines();

    while let Some(line) = lines.next() {
        if line.contains(line_to_check) {
            println!("Found line: {}", line);
            // Check if the next line also contains the line_to_check
            if let Some(next_line) = lines.next() {
                if next_line.contains(line_to_check) {
                    println!("Found next line: {}", next_line);
                    return true;
                }
            }
        }
    }

    false
}
