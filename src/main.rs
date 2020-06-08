//! # Codewars Bot (for Slack)

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
use structopt::clap::AppSettings;
use structopt::StructOpt;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::sync::Mutex;

mod api;
mod commands;
mod scheduling;
mod server;
mod storage;

use crate::api::slack::event::AppMention;
use crate::api::{codewars, slack};
use crate::commands::Command;
use crate::storage::Repository;

const SETTINGS_FILE: &str = "settings.toml";

#[derive(Debug, StructOpt)]
#[structopt(about, author, setting = AppSettings::ColoredHelp)]
struct Opt {
    /// Port to listen for connections.
    #[structopt(long, env, default_value = "8080")]
    port: u16,
    /// Signing key to verify HTTP calls come from Slack.
    #[structopt(long, env, hide_env_values = true)]
    signing_key: String,
    /// Webhook URL to post messages to a Slack channel.
    #[structopt(long, env, hide_env_values = true)]
    webhook_url: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();

    let opt: Opt = Opt::from_args();

    setup_logger()?;

    run_server(opt.port, opt.signing_key, opt.webhook_url).await?;

    Ok(())
}

fn setup_logger() -> Result<()> {
    let colored = |l: log::Level| -> console::StyledObject<log::Level> {
        let styled = console::style(l);
        match l {
            log::Level::Trace => styled.magenta(),
            log::Level::Debug => styled.blue(),
            log::Level::Info => styled.green(),
            log::Level::Warn => styled.yellow(),
            log::Level::Error => styled.red(),
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
                        console::style(record.target()).bold(),
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
        match stats(&self.repo, None).await {
            Ok(msg) => webhook_send(&self.webhook_url, &msg).await,
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
    s_tx.send(msg)?;

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
    s_tx: UnboundedSender<(Weekday, NaiveTime)>,
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

        let (filter, n): (ChallengeFilter, usize) = if let Some(date) = since {
            (
                Box::new(move |c| c.completed_at.naive_local() >= date),
                usize::max_value(),
            )
        } else {
            (Box::new(|_| true), 3)
        };

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
    s_tx: &UnboundedSender<(Weekday, NaiveTime)>,
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
            s_tx.send((weekday, time)).ok();
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
