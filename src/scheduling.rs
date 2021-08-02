//! Schedulers to execute tasks on a fixed basis with the option to reschedule any time.

use async_trait::async_trait;
use chrono::{prelude::*, Duration, Local, NaiveTime, Weekday};
use futures::prelude::*;
use log::{debug, trace};
use tokio::{sync::mpsc::UnboundedReceiver, time::Duration as TokioDuration};

/// A task that is to be executed. It is used together with a [`Scheduler`] in the [`run`] function
/// to run any task on a fixed schedule.
#[async_trait]
pub trait Task: Send + Sync {
    /// Short title for the task to identify it in logs.
    fn name() -> &'static str;

    /// The logic that a task should execute.
    async fn run(&self);
}

/// Create an endless schedule for a given task. The task is executed regularly based on the rules
/// of a [`Scheduler`]. The schedule can be updated any time by sending new inputs through the
/// provided channel.
#[allow(clippy::cast_possible_wrap, clippy::cast_sign_loss)]
pub async fn run<S, T>(mut rx: UnboundedReceiver<Option<S::Input>>, task: T)
where
    S: Scheduler,
    T: Task,
{
    let mut schedule = None;
    let (delayed, mut handle) = future::abortable(future::pending());
    let mut delayed = delayed.boxed().shared();

    loop {
        schedule = tokio::select! {
            _ = delayed.clone() => {
                trace!("Executing {} task", T::name());
                task.run().await;

                schedule
            },
            Some(Some(s)) = rx.recv() => {
                trace!("Got new {} schedule", T::name());
                handle.abort();

                schedule = Some(s);
                schedule
            }
        };

        if let Some(schedule) = schedule {
            let duration = S::next(schedule);
            let duration = if let Ok(d) = duration.to_std() {
                d
            } else {
                TokioDuration::from_secs(duration.num_seconds() as u64)
            };

            let (d, h) = future::abortable(tokio::time::sleep(duration));
            delayed = d.boxed().shared();
            handle = h;

            debug!(
                "Next scheduled {} task in {} ({})",
                T::name(),
                humantime::Duration::from(duration),
                Local::now()
                    + if let Ok(d) = Duration::from_std(duration) {
                        d
                    } else {
                        Duration::seconds(duration.as_secs() as i64)
                    }
            );
        } else {
            let (d, h) = future::abortable(future::pending());
            delayed = d.boxed().shared();
            handle = h;

            debug!("Schedule for {} disabled", T::name());
        }
    }
}

/// A scheduler that calculates the duration to wait until next occurrence of an event. It is
/// generic over the input data which allows to create schedules out of any kind of data.
pub trait Scheduler: Send {
    type Input: Copy + Send;

    /// Calculate the wait duration until the next event should be triggered.
    fn next(input: Self::Input) -> Duration;
}

/// A scheduler that schedules events on a fixed weekday and time.
pub struct WeeklyScheduler;

impl Scheduler for WeeklyScheduler {
    type Input = (Weekday, NaiveTime);

    fn next((weekday, time): Self::Input) -> Duration {
        let now = Local::now().naive_local();
        let mut next = now.date();

        if now.weekday() == weekday && now.time() >= time {
            next += Duration::weeks(1);
        } else {
            while next.weekday() != weekday {
                next = next.succ();
            }
        }

        next.and_time(time) - now
    }
}

/// A scheduler that schedules events on a hour basis.
pub struct HourlyScheduler;

impl Scheduler for HourlyScheduler {
    type Input = u8;

    fn next(duration: Self::Input) -> Duration {
        Duration::hours(duration.into())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Once;

    use tokio::sync::mpsc;

    use super::*;

    static INIT: Once = Once::new();

    struct FakeScheduler;

    impl Scheduler for FakeScheduler {
        type Input = ();

        fn next(_: Self::Input) -> Duration {
            Duration::milliseconds(50)
        }
    }

    struct FakeTask;

    #[async_trait]
    impl Task for FakeTask {
        fn name() -> &'static str {
            "fake"
        }

        async fn run(&self) {
            debug!("fake task");
        }
    }

    fn init() {
        INIT.call_once(|| {
            std::env::set_var("RUST_LOG", "codewars_bot");
            env_logger::try_init().ok();
        });
    }

    #[tokio::test]
    async fn test_usage() {
        init();

        let (tx, rx) = mpsc::unbounded_channel();

        tokio::spawn(run::<FakeScheduler, _>(rx, FakeTask));

        tx.send(Some(())).unwrap();

        tokio::time::sleep(TokioDuration::from_millis(110)).await;
    }
}
