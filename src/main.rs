#![forbid(unsafe_code)]
#![deny(clippy::all, clippy::pedantic)]
#![warn(clippy::nursery)]
#![allow(dead_code)]

use std::fmt::Write;

use anyhow::Result;
use chrono::{NaiveDate, NaiveTime, Weekday};
use log::{error, info};
use structopt::clap::AppSettings;
use structopt::StructOpt;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

mod api;
mod commands;
mod scheduling;
mod storage;

use crate::api::slack::event::AppMention;
use crate::api::{codewars, slack};
use crate::commands::Command;
use crate::storage::Repository;

const SETTINGS_FILE: &str = "settings.toml";

#[derive(Debug, StructOpt)]
#[structopt(setting = AppSettings::ColoredHelp)]
struct Opt {
    #[structopt(subcommand)]
    cmd: Option<Subcommand>,
    #[structopt(long, env, hide_env_values = true)]
    app_token: String,
    #[structopt(long, env, hide_env_values = true)]
    bot_token: String,
    #[structopt(long, env, hide_env_values = true)]
    signing_key: String,
    #[structopt(long, env, hide_env_values = true)]
    webhook_url: String,
}

#[derive(Debug, StructOpt)]
enum Subcommand {
    /// Test the current settings for debugging.
    #[structopt(setting = AppSettings::ColoredHelp)]
    TestSettings,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();

    let opt: Opt = Opt::from_args();

    setup_logger()?;

    if let Some(cmd) = opt.cmd {
        match cmd {
            Subcommand::TestSettings => test_settings().await?,
        }
        return Ok(());
    }

    run_server(opt.signing_key, opt.webhook_url).await?;

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

async fn test_settings() -> Result<()> {
    let settings = Repository::load(SETTINGS_FILE).await?;
    for user in settings.users() {
        info!("loading codewars user data for `{}`", user);
        tokio::try_join!(
            codewars::user(user),
            codewars::completed_challenges(user),
            codewars::authored_challenges(user),
        )?;
    }

    Ok(())
}

use std::sync::Arc;

use tokio::sync::Mutex;

struct StatsTask(Arc<Mutex<Repository>>);

#[async_trait::async_trait]
impl<'a> scheduling::Task for StatsTask {
    async fn run(&self) {
        stats(&self.0, None).await.ok();
    }
}

async fn run_server(signing_key: String, webhook_url: String) -> Result<()> {
    let settings = Repository::load(SETTINGS_FILE).await?;
    let settings = Arc::new(Mutex::new(settings));
    let (tx, rx) = mpsc::unbounded_channel();

    let (s_tx, s_rx) = mpsc::unbounded_channel();
    tokio::spawn(scheduling::run::<scheduling::WeeklyScheduler, _>(
        s_rx,
        StatsTask(settings.clone()),
    ));

    let msg = {
        let l = settings.lock().await;
        let s = l.schedule();
        (s.weekday, s.time)
    };
    s_tx.send(msg)?;

    let server = tokio::spawn(slack::event::run_server(signing_key, tx));
    let handler = tokio::spawn(handle_events(webhook_url, settings.clone(), rx, s_tx));

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
                Command::Stats(since) => stats(&settings, since).await,
                Command::Help => help().await,
                Command::Schedule(weekday, time) => schedule(&settings, &s_tx, weekday, time).await,
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

async fn stats(settings: &Arc<Mutex<Repository>>, since: Option<NaiveDate>) -> Result<String> {
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
                Box::new(move |c| c.completed_at.date().naive_local() >= date),
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
