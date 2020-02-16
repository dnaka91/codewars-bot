#![forbid(unsafe_code)]
#![deny(clippy::all, clippy::pedantic)]
#![warn(clippy::nursery)]
#![allow(dead_code)]

use std::fmt::Write;

use anyhow::Result;
use chrono::{NaiveTime, Weekday};
use log::info;
use structopt::clap::AppSettings;
use structopt::StructOpt;
use tokio::sync::mpsc;

mod api;
mod commands;
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
}

#[derive(Debug, StructOpt)]
enum Subcommand {
    /// List all available channels with their corresponding ID.
    #[structopt(setting = AppSettings::ColoredHelp)]
    Channels,
    /// Test the current settings for debugging.
    #[structopt(setting = AppSettings::ColoredHelp)]
    TestSettings,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv()?;
    pretty_env_logger::try_init()?;

    let opt: Opt = Opt::from_args();

    if let Some(cmd) = opt.cmd {
        match cmd {
            Subcommand::Channels => list_channels().await?,
            Subcommand::TestSettings => test_settings().await?,
        }
        return Ok(());
    }

    run_server().await?;

    Ok(())
}

async fn list_channels() -> Result<()> {
    let mut channels = slack::web::users_conversations().await?;
    channels.sort_by(|a, b| a.name.cmp(&b.name));

    for channel in channels {
        println!("{} {}", channel.id, channel.name);
    }

    Ok(())
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

async fn run_server() -> Result<()> {
    let mut settings = Repository::load(SETTINGS_FILE).await?;
    let (tx, mut rx) = mpsc::unbounded_channel();

    let handle = tokio::spawn(slack::event::run_server(tx));

    while let Some(AppMention { user, text, .. }) = rx.recv().await {
        let prefix = if let Some(idx) = text.find("> ") {
            idx + 2
        } else {
            slack::webhook::send(&format!("<@{}>messages must start with a metion", user)).await?;
            continue;
        };

        let response = match commands::parse(&text[prefix..]) {
            Ok(cmd) => match cmd {
                Command::AddUser(username) => add_user(&mut settings, username).await,
                Command::RemoveUser(username) => remove_user(&mut settings, username).await,
                Command::Stats => stats(&settings).await,
                Command::Help => help().await,
                Command::Schedule(weekday, time) => schedule(&mut settings, weekday, time).await,
            }?,
            Err(e) => format!("Unknown command:\n```{}```", e),
        };
        slack::webhook::send(&response).await?;
    }

    handle.await?;
    Ok(())
}

async fn add_user(settings: &mut Repository, username: String) -> Result<String> {
    Ok(if settings.add_user(&username).await? {
        format!("Added user `{}` to watchlist", username)
    } else {
        format!("User `{}` is already in the watchlist", username)
    })
}

async fn remove_user(settings: &mut Repository, username: String) -> Result<String> {
    Ok(if settings.remove_user(&username).await? {
        format!("Removed user `{}` from watchlist", username)
    } else {
        format!("User `{}` is not in the watchlist", username)
    })
}

async fn stats(settings: &Repository) -> Result<String> {
    let mut response = String::from("Here are the current statistics:");
    for user in settings.users() {
        let challenge_resp = codewars::completed_challenges(user).await?;
        let mut challenges = challenge_resp.data;
        challenges.sort_by(|a, b| a.completed_at.cmp(&b.completed_at));
        challenges.reverse();

        write!(
            &mut response,
            "\n\n`{}` - {} total challenges",
            user, challenge_resp.total_items
        )?;

        for challenge in challenges.into_iter().take(3) {
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

Here are all the commands I know:
- `add <user>`: Add a Codewars user to the statistics report.
- `remove <user>`: Remove a Codewars user from the statistics again.
- `stats`: Show the current statistics of all tracked users.
- `help`: Show this help.",
    ))
}

async fn schedule(settings: &mut Repository, weekday: Weekday, time: NaiveTime) -> Result<String> {
    Ok(
        if settings
            .set_schedule(storage::Schedule { weekday, time })
            .await?
        {
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
