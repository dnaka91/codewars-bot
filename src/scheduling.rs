use async_trait::async_trait;
use chrono::prelude::*;
use chrono::Duration;
use chrono::{Local, NaiveTime, Weekday};
use futures::prelude::*;
use log::{debug, trace};
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::Duration as TokioDuration;

#[async_trait]
pub trait Task {
    fn name() -> &'static str;

    async fn run(&self);
}

#[allow(clippy::cast_possible_wrap, clippy::cast_sign_loss)]
pub async fn run<S, T>(mut rx: UnboundedReceiver<S::Input>, task: T)
where
    S: Scheduler,
    T: Task,
{
    let mut duration = None;
    let (delayed, mut handle) = future::abortable(future::pending());
    let mut delayed = delayed.boxed().shared();

    loop {
        duration = tokio::select! {
            _ = delayed.clone() => {
                trace!("Executing {} task", T::name());
                task.run().await;

                duration
            },
            Some(schedule) = rx.recv() => {
                trace!("Got new {} schedule", T::name());
                handle.abort();

                let duration = S::next(schedule);

                duration.map(|d| if let Ok(d) = d.to_std(){
                    d
                } else{
                    TokioDuration::from_secs(d.num_seconds() as u64)
                })
            }
        };

        if let Some(duration) = duration {
            let (d, h) = future::abortable(tokio::time::delay_for(duration));
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

            debug!("Schedule for {} disabled", T::name())
        }
    }
}

pub trait Scheduler {
    type Input;

    fn next(input: Self::Input) -> Option<Duration>;
}

pub struct WeeklyScheduler;

impl Scheduler for WeeklyScheduler {
    type Input = (Weekday, NaiveTime);

    fn next((weekday, time): Self::Input) -> Option<Duration> {
        let now = Local::now().naive_local();
        let mut next = now.date();

        if next.weekday() == weekday {
            next += Duration::weeks(1);
        } else {
            while next.weekday() != weekday {
                next = next.succ();
            }
        }

        Some(next.and_time(time) - now)
    }
}

pub struct HourlyScheduler;

impl Scheduler for HourlyScheduler {
    type Input = Option<u8>;

    fn next(duration: Self::Input) -> Option<Duration> {
        duration.map(|d| Duration::hours(i64::from(d)))
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

        fn next(_: Self::Input) -> Option<Duration> {
            Some(Duration::milliseconds(50))
        }
    }

    struct FakeTask;

    #[async_trait]
    impl Task for FakeTask {
        fn name() -> &'static str {
            "fake"
        }

        async fn run(&self) {
            debug!("fake task")
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

        tx.send(()).unwrap();

        tokio::time::delay_for(TokioDuration::from_millis(110)).await;
    }
}
