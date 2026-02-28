//! Integration tests for `arbiter-core` exercising the full actor lifecycle,
//! multi-actor communication, and snapshot streaming — all using the shared
//! fixtures (`PingPlayer`, `PongPlayer`, `Counter`, `Ping`, `Pong`).

use arbiter_core::{fixtures::*, network::memory::InMemory, runtime::Runtime};
use tokio_stream::StreamExt;

/// Two actors exchange Ping/Pong messages. `PingPlayer` drives the exchange
/// and self-stops at `max_count`. Verify the full snapshot stream matches the
/// expected sequence and closes when the actor self-stops.
#[tokio::test]
async fn ping_pong_self_stop() {
  let runtime = Runtime::<InMemory>::new();

  let ping =
    runtime.spawn(PingPlayer { count: 0, max_count: 10 }).with_handler::<Pong>().with_name("ping");

  let pong = runtime.spawn(PongPlayer).with_handler::<Ping>().with_name("pong");

  let mut ping = ping.process();
  let mut snapshots = ping.stream().unwrap();
  ping.start().await.unwrap();

  let mut pong = pong.process();
  pong.start().await.unwrap();

  // PingPlayer: initial=0, increments on each Pong, stops at max_count
  for expected in 0..=10 {
    assert_eq!(snapshots.next().await.unwrap(), expected);
  }

  // Stream closes after self-stop
  assert_eq!(snapshots.next().await, None);
}

/// A `Counter` observes all broadcast messages on the network during a
/// ping-pong exchange. It should count every `Ping` and `Pong` that
/// passes through the broadcast channel.
#[tokio::test]
async fn observer_counts_broadcast_messages() {
  let runtime = Runtime::<InMemory>::new();

  let ping =
    runtime.spawn(PingPlayer { count: 0, max_count: 5 }).with_handler::<Pong>().with_name("ping");

  let pong = runtime.spawn(PongPlayer).with_handler::<Ping>().with_name("pong");

  let observer = runtime
    .spawn(Counter { count: 0 })
    .with_handler::<Ping>()
    .with_handler::<Pong>()
    .with_name("observer");

  let mut ping = ping.process();
  let mut pong = pong.process();
  let mut observer = observer.process();
  let mut observer_snapshots = observer.stream().unwrap();

  ping.start().await.unwrap();
  pong.start().await.unwrap();
  observer.start().await.unwrap();

  // Initial observer snapshot is 0
  assert_eq!(observer_snapshots.next().await.unwrap(), 0);

  // PingPlayer sends 6 Pings (on_start + 5 handler replies) and
  // PongPlayer sends 6 Pongs = 12 total messages observed.
  let mut last = 0;
  while last < 12 {
    last = observer_snapshots.next().await.unwrap();
  }
  assert_eq!(last, 12);

  observer.stop().await.unwrap();
}

/// Verify snapshot streams from two different actors simultaneously.
/// `PingPlayer` produces count snapshots; `Counter` (observing) produces
/// its own independent count snapshots.
#[tokio::test]
async fn dual_snapshot_streams() {
  let runtime = Runtime::<InMemory>::new();

  let ping =
    runtime.spawn(PingPlayer { count: 0, max_count: 5 }).with_handler::<Pong>().with_name("ping");

  let pong = runtime.spawn(PongPlayer).with_handler::<Ping>().with_name("pong");

  let observer = runtime
    .spawn(Counter { count: 0 })
    .with_handler::<Ping>()
    .with_handler::<Pong>()
    .with_name("observer");

  let mut ping = ping.process();
  let mut pong = pong.process();
  let mut observer = observer.process();

  let mut ping_snapshots = ping.stream().unwrap();
  let mut observer_snapshots = observer.stream().unwrap();

  ping.start().await.unwrap();
  pong.start().await.unwrap();
  observer.start().await.unwrap();

  // PingPlayer snapshots: 0, 1, 2, 3, 4, 5 then stream closes
  for expected in 0..=5 {
    assert_eq!(ping_snapshots.next().await.unwrap(), expected);
  }
  assert_eq!(ping_snapshots.next().await, None);

  // Observer should have seen messages — its count should be > 0
  // Initial snapshot is 0
  assert_eq!(observer_snapshots.next().await.unwrap(), 0);

  // Drain observer snapshots — final count should be 12
  // (6 Pings + 6 Pongs on the broadcast network)
  let mut last = 0;
  while last < 12 {
    last = observer_snapshots.next().await.unwrap();
  }
  assert_eq!(last, 12);

  observer.stop().await.unwrap();
}
