//! # Codewars Bot for Slack
//!
//! This service is a bot for Slack that connects to [Codewars](https://codewars.com) to show
//! statistic about a user selected list of users.
//!
//! It can show statistics at any time, but also features a fixed schedule to report the latest
//! stats without user intervention. Lastly it can give a notification whenever a new challenge was
//! completed by one of the tracked users.
//!
//! ## Slack commands
//
//! The service currently knows all the following commands that can be triggered by sending a Slack
//! message with `@<botname> <command>`:
//!
//! ### `add <user>`
//!
//! Add a Codewars user to the statistics report.
//!
//! ### `remove <user>`
//!
//! Remove a Codewars user from the statistics again.
//!
//! ### `stats [since <date>]`
//!
//! Show the current statistics of all tracked users.
//! - The format of `<date>` is `YYYY/MM/DD`, for example `2020/02/12` or `2020/1/2`.
//! - The date is optional.
//!
//! ### `schedule on <weekday> [at <time>]`
//!
//! Set a weekly schedule to send the latest stats.
//! - The format of `<weekday>` is the weekday name in short or long form, for example `wed` or `Friday`.
//! - The format of `<time>` is `HH:MM`, for example `12:25` or `01:00`.
//! - The time is optional and defaults to `10:00`.
//!
//! ### `notify <on|off>`
//!
//! Send notifications whenever new challenges are completed.
//!
//! ### `help`
//!
//! Show information about all available commands.

#![forbid(unsafe_code)]
#![deny(clippy::all, clippy::pedantic)]
#![warn(clippy::nursery)]
#![allow(clippy::used_underscore_binding, clippy::wildcard_imports)]

use std::fmt::Write;
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use chrono::prelude::*;
use chrono::Duration;
use log::error;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::sync::Mutex;

mod api;
mod commands;
mod scheduling;
mod server;
mod settings;
mod storage;

use crate::api::slack::event::AppMention;
use crate::api::{codewars, slack};
use crate::commands::Command;
use crate::storage::Repository;

const SETTINGS_FILE: &str = "settings.toml";

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();

    let opt = settings::load()?;

    setup_logger()?;

    run_server(opt.port, opt.signing_key, opt.webhook_url).await?;

    Ok(())
}

fn setup_logger() -> Result<()> {
    use yansi::Paint;

    let colored = |l: log::Level| -> Paint<log::Level> {
        match l {
            log::Level::Trace => Paint::magenta(l),
            log::Level::Debug => Paint::blue(l),
            log::Level::Info => Paint::green(l),
            log::Level::Warn => Paint::yellow(l),
            log::Level::Error => Paint::red(l),
        }
    };

    fern::Dispatch::new()
        .chain(
            fern::Dispatch::new()
                .format(move |out, message, record| {
                    out.finish(format_args!(
                        "[{}] [{:5}] [{}] {}",
                        chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                        record.level(),
                        record.target(),
                        message
                    ))
                })
                .level(log::LevelFilter::Info)
                .chain(fern::log_file("codewars-bot.log")?),
        )
        .chain(
            fern::Dispatch::new()
                .format(move |out, message, record| {
                    out.finish(format_args!(
                        "{} {:5} {} > {}",
                        chrono::Local::now().format("%H:%M:%S"),
                        colored(record.level()),
                        Paint::new(record.target()).bold(),
                        message
                    ))
                })
                .level(log::LevelFilter::Info)
                .level_for("codewars_bot", log::LevelFilter::Trace)
                .level_for("server", log::LevelFilter::Trace)
                .chain(std::io::stdout()),
        )
        .apply()
        .map_err(Into::into)
}

struct StatsTask {
    repo: Arc<Mutex<Repository>>,
    webhook_url: String,
}

#[async_trait]
impl<'a> scheduling::Task for StatsTask {
    fn name() -> &'static str {
        "stats"
    }

    async fn run(&self) {
        let start_time = Utc::now();
        let since = self.repo.lock().await.last_run().map(|dt| dt.naive_local());

        match stats(&self.repo, since).await {
            Ok(msg) => {
                webhook_send(&self.webhook_url, &msg).await;
                if let Err(e) = self.repo.lock().await.set_last_run(start_time).await {
                    error!("Error saving last run time: {}", e);
                }
            }
            Err(e) => error!("Error collecting scheduled stats: {}", e),
        }
    }
}

struct NotifyTask {
    repo: Arc<Mutex<Repository>>,
    webhook_url: String,
}

#[async_trait]
impl<'a> scheduling::Task for NotifyTask {
    fn name() -> &'static str {
        "notify"
    }

    async fn run(&self) {
        match stats(
            &self.repo,
            Some(Local::now().naive_local() - Duration::hours(3)),
        )
        .await
        {
            Ok(msg) => webhook_send(&self.webhook_url, &msg).await,
            Err(e) => error!("Error collecting stats for notification: {}", e),
        }
    }
}

async fn run_server(port: u16, signing_key: String, webhook_url: String) -> Result<()> {
    let settings = Repository::load(SETTINGS_FILE).await?;
    let settings = Arc::new(Mutex::new(settings));
    let (tx, rx) = mpsc::unbounded_channel();

    let (s_tx, s_rx) = mpsc::unbounded_channel();
    tokio::spawn(scheduling::run::<scheduling::WeeklyScheduler, _>(
        s_rx,
        StatsTask {
            repo: settings.clone(),
            webhook_url: webhook_url.clone(),
        },
    ));

    let msg = {
        let l = settings.lock().await;
        let s = l.schedule();
        (s.weekday, s.time)
    };
    s_tx.send(Some(msg))?;

    let (n_tx, n_rx) = mpsc::unbounded_channel();
    tokio::spawn(scheduling::run::<scheduling::HourlyScheduler, _>(
        n_rx,
        NotifyTask {
            repo: settings.clone(),
            webhook_url: webhook_url.clone(),
        },
    ));

    let msg = {
        let l = settings.lock().await;
        l.notify()
    };
    if msg {
        n_tx.send(Some(3))?;
    }

    let server = tokio::spawn(server::run(port, signing_key, tx));
    let handler = tokio::spawn(handle_events(webhook_url, settings.clone(), rx, s_tx, n_tx));

    tokio::select! {
        res = server => res?,
        _ = handler => ()
    }

    Ok(())
}

async fn handle_events(
    webhook_url: String,
    settings: Arc<Mutex<Repository>>,
    mut rx: UnboundedReceiver<AppMention>,
    s_tx: UnboundedSender<Option<(Weekday, NaiveTime)>>,
    n_tx: UnboundedSender<Option<u8>>,
) {
    while let Some(AppMention { user, text, .. }) = rx.recv().await {
        let prefix = if let Some(idx) = text.find("> ") {
            idx + 2
        } else {
            webhook_send(
                &webhook_url,
                &format!("<@{}> messages must start with a mention", user),
            )
            .await;
            continue;
        };

        let response = match commands::parse(&text[prefix..]) {
            Ok(cmd) => match cmd {
                Command::AddUser(username) => add_user(&settings, username).await,
                Command::RemoveUser(username) => remove_user(&settings, username).await,
                Command::Stats(since) => stats(&settings, since.map(|d| d.and_hms(0, 0, 0))).await,
                Command::Help => help().await,
                Command::Schedule(weekday, time) => schedule(&settings, &s_tx, weekday, time).await,
                Command::Notify(on_off) => notify(&settings, &n_tx, on_off).await,
            },
            Err(e) => Ok(format!("Unknown command:\n```{}```", e)),
        };

        match response {
            Ok(message) => webhook_send(&webhook_url, &message).await,
            Err(e) => {
                error!("Error during command processing: {}", e);
                webhook_send(
                    &webhook_url,
                    &format!(
                        "Sorry <@{}>, something went wrong while processing your command",
                        user
                    ),
                )
                .await
            }
        }
    }
}

async fn webhook_send(webhook_url: &str, text: &str) {
    if let Err(e) = slack::webhook::send(webhook_url, text).await {
        error!("Error during message sending by webhook: {}", e);
    }
}

async fn add_user(settings: &Arc<Mutex<Repository>>, username: String) -> Result<String> {
    Ok(if settings.lock().await.add_user(&username).await? {
        format!("Added user `{}` to watchlist", username)
    } else {
        format!("User `{}` is already in the watchlist", username)
    })
}

async fn remove_user(settings: &Arc<Mutex<Repository>>, username: String) -> Result<String> {
    Ok(if settings.lock().await.remove_user(&username).await? {
        format!("Removed user `{}` from watchlist", username)
    } else {
        format!("User `{}` is not in the watchlist", username)
    })
}

async fn stats(settings: &Arc<Mutex<Repository>>, since: Option<NaiveDateTime>) -> Result<String> {
    use codewars::CompletedChallenge;

    type ChallengeFilter = Box<dyn FnMut(&CompletedChallenge) -> bool>;

    let mut response = String::from("Here are the current statistics:");
    for user in settings.lock().await.users() {
        let challenge_resp = codewars::completed_challenges(user).await?;
        let mut challenges = challenge_resp.data;
        challenges.sort_by(|a, b| a.completed_at.cmp(&b.completed_at));
        challenges.reverse();

        write!(
            &mut response,
            "\n\n`{}` - {} total challenges",
            user, challenge_resp.total_items
        )?;

        let (filter, n): (ChallengeFilter, usize) = since.map_or((Box::new(|_| true), 3), |date| {
            (
                Box::new(move |c| c.completed_at.naive_local() >= date),
                usize::max_value(),
            )
        });

        for challenge in challenges.into_iter().filter(filter).take(n) {
            if let Some(name) = challenge.name {
                write!(
                    &mut response,
                    "\n*{}* solved at _{}_ in *{}*",
                    name,
                    challenge.completed_at.format("%Y/%m/%d"),
                    challenge
                        .completed_languages
                        .into_iter()
                        .collect::<Vec<_>>()
                        .join(", ")
                )?;
            }
        }
    }

    Ok(response)
}

async fn help() -> Result<String> {
    Ok(String::from(
        "\
Hello there, I'm a Codewars bot. You can use me by mentioning me, followed by a command.
For example `@codewarsbot stats`.

*Here are all the commands I know:*

```add <user>```
 Add a Codewars user to the statistics report.

```remove <user>```
Remove a Codewars user from the statistics again.

```stats [since <date>]```
Show the current statistics of all tracked users.
- The format of `<date>` is `YYYY/MM/DD`, for example `2020/02/12` or `2020/1/2`.
- The date is optional.

```schedule on <weekday> [at <time>]```
Set a weekly schedule to send the latest stats.
- The format of `<weekday>` is the weekday name in short or long form, for example `wed` or `Friday`.
- The format of `<time>` is `HH:MM`, for example `12:25` or `01:00`.
- The time is optional and defaults to `10:00`.

```notify <on|off>```
Send notifications whenever new challenges are completed.

```help```
Show this help.",
    ))
}

async fn schedule(
    settings: &Arc<Mutex<Repository>>,
    s_tx: &UnboundedSender<Option<(Weekday, NaiveTime)>>,
    weekday: Weekday,
    time: NaiveTime,
) -> Result<String> {
    Ok(
        if settings
            .lock()
            .await
            .set_schedule(storage::Schedule { weekday, time })
            .await?
        {
            s_tx.send(Some((weekday, time))).ok();
            format!(
                "Weekly schedule updated to send stats on `{}s` at `{}`",
                match weekday {
                    Weekday::Mon => "Monday",
                    Weekday::Tue => "Tuesday",
                    Weekday::Wed => "Wednesday",
                    Weekday::Thu => "Thursday",
                    Weekday::Fri => "Friday",
                    Weekday::Sat => "Saturday",
                    Weekday::Sun => "Sunday",
                },
                time
            )
        } else {
            String::from("Weekly schedule already set to this weekday & time")
        },
    )
}

async fn notify(
    settings: &Arc<Mutex<Repository>>,
    n_tx: &UnboundedSender<Option<u8>>,
    on_off: bool,
) -> Result<String> {
    Ok(if settings.lock().await.set_notify(on_off).await? {
        let msg = if on_off { Some(3) } else { None };
        n_tx.send(msg).ok();
        format!(
            "Notifications {}",
            if on_off { "enabled" } else { "disabled" }
        )
    } else {
        format!(
            "Notifications already {}",
            if settings.lock().await.notify() {
                "enabled"
            } else {
                "disabled"
            }
        )
    })
}
