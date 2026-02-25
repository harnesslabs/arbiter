# Replay Guide

Replay support is feature-gated and reuses structured observability events captured during LAN or local runs.

## Features

Enable `replay` (and `tcp` if your run is networked):

```toml
[dependencies]
arbiter = { version = "0.5", features = ["replay", "tcp"] }
```

## Recording Events

Use `Observer` with an in-memory recorder or JSONL sink:

```rust,ignore
use arbiter::observe::{JsonlFileSink, Observer};

let observer = Observer::new();
observer.add_sink(JsonlFileSink::create("arbiter-events.jsonl")?);
```

Attach the observer to broker and/or node runtime components:

```rust,ignore
use arbiter::runtime::broker::BrokerConfig;

let broker_config = BrokerConfig::new(([127,0,0,1], 0).into(), "broker-1")
    .with_observer(observer.clone());
```

## Replaying Recorded Order

```rust,ignore
use arbiter::replay::{ReplayHarness, ReplayLog};

let log = ReplayLog::read_jsonl("arbiter-events.jsonl")?;
ReplayHarness::new().replay_recorded_order(&log)?;
```

## What Replay Guarantees

- preserved recorded event order
- validation of expected event kind sequence / routing outcomes
- useful mismatch reporting for debugging regressions

## What Replay Does Not Guarantee (Yet)

- full distributed determinism
- reproduction of timing/race behavior from live network runs
