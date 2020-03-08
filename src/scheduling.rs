use async_trait::async_trait;
use chrono::prelude::*;
use chrono::Duration;
use chrono::{Local, NaiveTime, Weekday};
use futures::prelude::*;
use log::{debug, trace};
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::Duration as TokioDuration;

const DURATION_MAX: TokioDuration = TokioDuration::from_secs(100_000);

#[async_trait]
pub trait Task {
    async fn run(&self);
}

#[allow(clippy::cast_sign_loss)]
pub async fn run<S, T>(mut rx: UnboundedReceiver<S::Input>, task: T)
where
    S: Scheduler,
    T: Task,
{
    let mut duration = None;
    let (delayed, mut handle) = future::abortable(future::pending());
    let mut delayed = delayed.boxed().shared();

    loop {
        let new_duration = tokio::select! {
            _ = delayed.clone() => {
                trace!("Executing task");
                task.run().await;

                None
            },
            Some(schedule) = rx.recv() => {
                trace!("Got new schedule");
                handle.abort();

                let duration = S::next(schedule);
                let duration = if let Ok(d) = duration.to_std(){
                    d
                } else{
                    TokioDuration::from_secs(duration.num_seconds() as u64)
                };

                Some(duration)
            }
        };

        if let Some(new_duration) = new_duration {
            duration = Some(new_duration)
        }

        if let Some(duration) = duration {
            let (d, h) = future::abortable(tokio::time::delay_for(duration));
            delayed = d.boxed().shared();
            handle = h;

            debug!("Next scheduled task: {}", humantime::Duration::from(duration));
        }
    }
}

pub trait Scheduler {
    type Input;

    fn next(input: Self::Input) -> Duration;
}

pub struct WeeklyScheduler;

impl Scheduler for WeeklyScheduler {
    type Input = (Weekday, NaiveTime);

    fn next((weekday, time): Self::Input) -> Duration {
        let now = Local::now().naive_local();
        let mut next = now.date();

        if next.weekday() == weekday {
            next += Duration::weeks(1);
        } else {
            while next.weekday() != weekday {
                next = next.succ();
            }
        }

        next.and_time(time) - now
    }
}

pub struct HourlyScheduler;

impl Scheduler for HourlyScheduler {
    type Input = i64;

    fn next(duration: Self::Input) -> Duration {
        Duration::hours(duration)
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
        async fn run(&self) {
            debug!("fake task")
        }
    }

    fn init() {
        INIT.call_once(|| {
            std::env::set_var("RUST_LOG", "codewars_bot");
            pretty_env_logger::try_init().ok();
        });
    }

    #[tokio::test]
    async fn test_usage() {
        init();

        let (tx, rx) = mpsc::unbounded_channel();

        tokio::spawn(run::<FakeScheduler, _>(rx, FakeTask));

        tx.send(()).unwrap();

        tokio::time::delay_for(TokioDuration::from_millis(110)).await;
    }
}
