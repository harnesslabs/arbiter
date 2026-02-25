use std::{
  fs::File,
  io::{BufRead, BufReader, Write},
  path::Path,
};

use thiserror::Error;

use crate::observe::ObserveEvent;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayLog {
  pub events: Vec<ObserveEvent>,
}

impl ReplayLog {
  pub fn new(events: Vec<ObserveEvent>) -> Self {
    Self { events }
  }

  pub fn from_jsonl_reader<R: BufRead>(reader: R) -> Result<Self, ReplayError> {
    let mut events = Vec::new();

    for (index, line_result) in reader.lines().enumerate() {
      let line = line_result.map_err(|source| ReplayError::Io { source })?;
      if line.trim().is_empty() {
        continue;
      }

      let event = serde_json::from_str::<ObserveEvent>(&line)
        .map_err(|source| ReplayError::JsonDecodeLine { line_number: index + 1, source })?;
      events.push(event);
    }

    Ok(Self { events })
  }

  pub fn from_jsonl_file(path: impl AsRef<Path>) -> Result<Self, ReplayError> {
    let file = File::open(path).map_err(|source| ReplayError::Io { source })?;
    Self::from_jsonl_reader(BufReader::new(file))
  }

  pub fn write_jsonl<W: Write>(&self, mut writer: W) -> Result<(), ReplayError> {
    for event in &self.events {
      serde_json::to_writer(&mut writer, event)
        .map_err(|source| ReplayError::JsonEncode { source })?;
      writer.write_all(b"\n").map_err(|source| ReplayError::Io { source })?;
    }
    Ok(())
  }

  pub fn write_jsonl_file(&self, path: impl AsRef<Path>) -> Result<(), ReplayError> {
    let file = File::create(path).map_err(|source| ReplayError::Io { source })?;
    self.write_jsonl(file)
  }
}

#[derive(Debug, Error)]
pub enum ReplayError {
  #[error("io error: {source}")]
  Io { source: std::io::Error },
  #[error("failed to decode replay jsonl line {line_number}: {source}")]
  JsonDecodeLine { line_number: usize, source: serde_json::Error },
  #[error("failed to encode replay event: {source}")]
  JsonEncode { source: serde_json::Error },
  #[error("replay mismatch at event {index}: {reason}")]
  ReplayMismatch { index: usize, reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{reason}")]
pub struct ReplayMismatch {
  pub reason: String,
}

impl ReplayMismatch {
  pub fn new(reason: impl Into<String>) -> Self {
    Self { reason: reason.into() }
  }
}

/// Recorded-order replay harness. This validates replay consumers against the exact observed order.
pub struct ReplayHarness;

impl ReplayHarness {
  pub fn replay_recorded_order<F>(log: &ReplayLog, mut validate: F) -> Result<(), ReplayError>
  where
    F: FnMut(usize, &ObserveEvent) -> Result<(), ReplayMismatch>,
  {
    for (index, event) in log.events.iter().enumerate() {
      validate(index, event)
        .map_err(|mismatch| ReplayError::ReplayMismatch { index, reason: mismatch.reason })?;
    }

    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use crate::observe::{ObserveEvent, ObserveEventKind};

  use super::*;

  #[test]
  fn replay_log_jsonl_round_trip_preserves_order() {
    let log = ReplayLog::new(vec![
      ObserveEvent::new(ObserveEventKind::BrokerNodeConnected { node_id: "node-a".into() }),
      ObserveEvent::new(ObserveEventKind::BrokerNodeConnected { node_id: "node-b".into() }),
    ]);

    let mut bytes = Vec::new();
    log.write_jsonl(&mut bytes).expect("write replay jsonl");

    let parsed =
      ReplayLog::from_jsonl_reader(std::io::Cursor::new(bytes)).expect("read replay jsonl");
    assert_eq!(parsed.events.len(), 2);

    match &parsed.events[0].kind {
      ObserveEventKind::BrokerNodeConnected { node_id } => assert_eq!(node_id.as_str(), "node-a"),
      other => panic!("unexpected event: {other:?}"),
    }
    match &parsed.events[1].kind {
      ObserveEventKind::BrokerNodeConnected { node_id } => assert_eq!(node_id.as_str(), "node-b"),
      other => panic!("unexpected event: {other:?}"),
    }
  }

  #[test]
  fn replay_harness_surfaces_mismatch() {
    let log = ReplayLog::new(vec![ObserveEvent::new(ObserveEventKind::BrokerNodeConnected {
      node_id: "node-a".into(),
    })]);

    let error = ReplayHarness::replay_recorded_order(&log, |_index, _event| {
      Err(ReplayMismatch::new("expected different event"))
    })
    .expect_err("expected replay mismatch");

    match error {
      ReplayError::ReplayMismatch { index, reason } => {
        assert_eq!(index, 0);
        assert_eq!(reason, "expected different event");
      },
      other => panic!("unexpected error: {other:?}"),
    }
  }
}
