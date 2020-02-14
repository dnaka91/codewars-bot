#![forbid(unsafe_code)]
#![deny(clippy::all, clippy::pedantic)]
#![warn(clippy::nursery)]
#![allow(dead_code)]

use std::env;
use std::fmt::Write;

use anyhow::{Context, Result};
use futures::prelude::*;
use log::info;
use structopt::clap::AppSettings;
use structopt::StructOpt;

mod codewars;
mod commands;
mod slack;
mod storage;

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

    run().await?;

    Ok(())
}

async fn list_channels() -> Result<()> {
    let mut channels = slack::users_conversations().await?;
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

async fn run() -> Result<()> {
    let mut settings = Repository::load(SETTINGS_FILE).await?;

    let bot_user = slack::users_list()
        .await?
        .into_iter()
        .find(|u| !u.deleted && u.is_bot && u.name == "codewarsbot")
        .context("bot user ID not found for `codewarsbot`")?;
    let prefix = format!("<@{}> ", bot_user.id);

    let (_, mut r) = slack::rtm_connect().await?;
    let target_channel = env::var("SLACK_CHANNEL")?;

    while let Some(event) = r.next().await {
        info!("EVENT {:?}", event);

        if let slack::Event::Message {
            channel,
            text,
            user,
            ..
        } = event
        {
            if channel == target_channel && text.starts_with(&prefix) {
                let response = match commands::parse(&text[prefix.len()..]) {
                    Ok(cmd) => match cmd {
                        Command::AddUser(username) => {
                            settings.add_user(&username).await?;
                            format!("Added user `{}` to watchlist", username)
                        }
                        Command::RemoveUser(username) => {
                            settings.remove_user(&username).await?;
                            format!("Removed user `{}` from watchlist", username)
                        }
                        Command::Stats => {
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

                            response
                        }
                        Command::Help => format!("Hey there <@{}>! This is still in TODO!", user),
                    },
                    Err(e) => format!("Unknown command: {}", e),
                };
                slack::webhook_message(&response).await?;
            }
        }
    }

    Ok(())
}
