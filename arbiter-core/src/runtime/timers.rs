use std::{future::Future, time::Duration};

use tokio::{
  task::{JoinError, JoinHandle},
  time::MissedTickBehavior,
};

/// Lightweight scheduling hooks for node-local periodic/one-shot work.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerSchedule {
  Once { delay: Duration },
  Interval { period: Duration },
}

/// Handle for a spawned timer task.
#[derive(Debug)]
pub struct TimerHandle {
  join: JoinHandle<()>,
}

impl TimerHandle {
  pub fn abort(&self) {
    self.join.abort();
  }

  pub fn is_finished(&self) -> bool {
    self.join.is_finished()
  }

  pub async fn join(self) -> Result<(), JoinError> {
    self.join.await
  }
}

pub fn spawn_once<F, Fut>(delay: Duration, task: F) -> TimerHandle
where
  F: FnOnce() -> Fut + Send + 'static,
  Fut: Future<Output = ()> + Send + 'static,
{
  TimerHandle {
    join: tokio::spawn(async move {
      tokio::time::sleep(delay).await;
      task().await;
    }),
  }
}

pub fn spawn_interval<F, Fut>(period: Duration, task: F) -> TimerHandle
where
  F: FnMut() -> Fut + Send + 'static,
  Fut: Future<Output = ()> + Send + 'static,
{
  spawn_schedule(TimerSchedule::Interval { period }, task)
}

pub fn spawn_schedule<F, Fut>(schedule: TimerSchedule, mut task: F) -> TimerHandle
where
  F: FnMut() -> Fut + Send + 'static,
  Fut: Future<Output = ()> + Send + 'static,
{
  let join = tokio::spawn(async move {
    match schedule {
      TimerSchedule::Once { delay } => {
        tokio::time::sleep(delay).await;
        task().await;
      },
      TimerSchedule::Interval { period } => {
        let period = period.max(Duration::from_millis(1));
        let mut interval = tokio::time::interval(period);
        interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
        loop {
          interval.tick().await;
          task().await;
        }
      },
    }
  });

  TimerHandle { join }
}

#[cfg(test)]
mod tests {
  use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
  };

  use tokio::time::{Duration, timeout};

  use super::*;

  #[tokio::test]
  async fn spawn_once_runs_callback() {
    let count = Arc::new(AtomicUsize::new(0));
    let count_for_task = Arc::clone(&count);
    let handle = spawn_once(Duration::from_millis(10), move || {
      let count = Arc::clone(&count_for_task);
      async move {
        count.fetch_add(1, Ordering::SeqCst);
      }
    });

    timeout(Duration::from_secs(1), handle.join())
      .await
      .expect("timer join timeout")
      .expect("join");
    assert_eq!(count.load(Ordering::SeqCst), 1);
  }

  #[tokio::test]
  async fn interval_timer_ticks_and_can_be_aborted() {
    let count = Arc::new(AtomicUsize::new(0));
    let count_for_task = Arc::clone(&count);
    let handle = spawn_interval(Duration::from_millis(10), move || {
      let count = Arc::clone(&count_for_task);
      async move {
        count.fetch_add(1, Ordering::SeqCst);
      }
    });

    timeout(Duration::from_secs(1), async {
      loop {
        if count.load(Ordering::SeqCst) >= 3 {
          break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
      }
    })
    .await
    .expect("interval ticks timeout");

    handle.abort();
    let _ = handle.join().await;
    let after_abort = count.load(Ordering::SeqCst);
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert_eq!(count.load(Ordering::SeqCst), after_abort);
  }
}
