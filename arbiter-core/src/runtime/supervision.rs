use std::time::Duration;

/// Restart/retry policy for node-local supervised tasks (agents, loops, adapters).
///
/// These primitives intentionally stay transport-agnostic so the node runtime can reuse them for
/// local agents and LAN-connected services without introducing a separate crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestartPolicy {
  Never,
  Immediate { max_restarts: usize },
  FixedDelay { max_restarts: usize, delay: Duration },
  ExponentialBackoff { max_restarts: usize, base_delay: Duration, max_delay: Duration },
}

impl Default for RestartPolicy {
  fn default() -> Self {
    Self::Never
  }
}

/// Supervisor decision emitted after a task failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupervisionDecision {
  Stop,
  RestartAfter(Duration),
}

/// Summary counters for supervision behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SupervisionStats {
  pub consecutive_failures: usize,
  pub total_failures: usize,
  pub total_restarts: usize,
}

/// Stateful supervision policy evaluator.
#[derive(Debug, Clone)]
pub struct Supervisor {
  policy: RestartPolicy,
  stats: SupervisionStats,
}

impl Supervisor {
  pub fn new(policy: RestartPolicy) -> Self {
    Self { policy, stats: SupervisionStats::default() }
  }

  pub const fn policy(&self) -> RestartPolicy {
    self.policy
  }

  pub const fn stats(&self) -> SupervisionStats {
    self.stats
  }

  /// Record a successful task run and reset the current failure streak.
  pub fn record_success(&mut self) {
    self.stats.consecutive_failures = 0;
  }

  /// Record a failure and return the next supervision action.
  pub fn record_failure(&mut self) -> SupervisionDecision {
    self.stats.total_failures += 1;
    self.stats.consecutive_failures += 1;
    let failure_ordinal = self.stats.consecutive_failures;

    let decision = match self.policy {
      RestartPolicy::Never => SupervisionDecision::Stop,
      RestartPolicy::Immediate { max_restarts } => {
        if failure_ordinal > max_restarts {
          SupervisionDecision::Stop
        } else {
          SupervisionDecision::RestartAfter(Duration::ZERO)
        }
      },
      RestartPolicy::FixedDelay { max_restarts, delay } => {
        if failure_ordinal > max_restarts {
          SupervisionDecision::Stop
        } else {
          SupervisionDecision::RestartAfter(delay)
        }
      },
      RestartPolicy::ExponentialBackoff { max_restarts, base_delay, max_delay } => {
        if failure_ordinal > max_restarts {
          SupervisionDecision::Stop
        } else {
          let multiplier =
            1u32.checked_shl((failure_ordinal.saturating_sub(1)) as u32).unwrap_or(u32::MAX);
          let delay = base_delay.saturating_mul(multiplier).min(max_delay);
          SupervisionDecision::RestartAfter(delay)
        }
      },
    };

    if matches!(decision, SupervisionDecision::RestartAfter(_)) {
      self.stats.total_restarts += 1;
    }

    decision
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn immediate_policy_stops_after_max_restarts() {
    let mut supervisor = Supervisor::new(RestartPolicy::Immediate { max_restarts: 2 });

    assert_eq!(supervisor.record_failure(), SupervisionDecision::RestartAfter(Duration::ZERO));
    assert_eq!(supervisor.record_failure(), SupervisionDecision::RestartAfter(Duration::ZERO));
    assert_eq!(supervisor.record_failure(), SupervisionDecision::Stop);

    let stats = supervisor.stats();
    assert_eq!(stats.total_failures, 3);
    assert_eq!(stats.total_restarts, 2);
    assert_eq!(stats.consecutive_failures, 3);
  }

  #[test]
  fn backoff_policy_caps_delay_and_resets_after_success() {
    let mut supervisor = Supervisor::new(RestartPolicy::ExponentialBackoff {
      max_restarts: 4,
      base_delay: Duration::from_millis(10),
      max_delay: Duration::from_millis(25),
    });

    assert_eq!(
      supervisor.record_failure(),
      SupervisionDecision::RestartAfter(Duration::from_millis(10))
    );
    assert_eq!(
      supervisor.record_failure(),
      SupervisionDecision::RestartAfter(Duration::from_millis(20))
    );
    assert_eq!(
      supervisor.record_failure(),
      SupervisionDecision::RestartAfter(Duration::from_millis(25))
    );

    supervisor.record_success();
    assert_eq!(supervisor.stats().consecutive_failures, 0);
    assert_eq!(
      supervisor.record_failure(),
      SupervisionDecision::RestartAfter(Duration::from_millis(10))
    );
  }
}
