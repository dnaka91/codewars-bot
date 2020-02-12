#![forbid(unsafe_code)]
#![deny(clippy::all, clippy::pedantic)]
#![warn(clippy::nursery)]

use std::env;

use anyhow::Result;
use futures::prelude::*;
use log::{debug, info};

mod codewars;
mod commands;
mod slack;
mod storage;

use crate::commands::Command;
use crate::storage::Repository;

const STARTER: &str = "!codewars-bot ";

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv()?;
    pretty_env_logger::try_init()?;

    for user in &[
        "dnaka91",
        "cschappert",
        "kitasuna",
        "gwoolhurme",
        "ddellacosta",
        "cdepillabout",
    ] {
        debug!("################### {} ###################", user);
        debug!(
            "{:#?}",
            tokio::try_join!(
                codewars::user(user),
                codewars::completed_challenges(user),
                codewars::authored_challenges(user),
            )?
        );
    }

    debug!(
        "{:#?}",
        codewars::code_challenge("multiples-of-3-or-5").await?
    );

    debug!("{:#?}", slack::users_conversations().await?);

    let mut settings = Repository::load("settings.toml").await?;

    let (_, mut r) = slack::rtm_connect().await?;
    let target_channel = env::var("SLACK_CHANNEL")?;

    while let Some(event) = r.next().await {
        info!("EVENT {:?}", event);

        if let slack::Event::Message { channel, text, .. } = event {
            if channel == target_channel && text.starts_with(STARTER) {
                let response = match commands::parse(&text[STARTER.len()..]) {
                    Ok(cmd) => match cmd {
                        Command::AddUser(username) => {
                            settings.add_user(&username).await?;
                            format!("Added user `{}` to watchlist", username)
                        }
                        Command::RemoveUser(username) => {
                            settings.remove_user(&username).await?;
                            format!("Removed user `{}` from watchlist", username)
                        }
                        Command::Stats => "Here are the current statistics: ...".to_owned(),
                    },
                    Err(e) => format!("Unknown command: {}", e),
                };
                slack::chat_post_message(&channel, &response).await?;
            }
        }
    }

    Ok(())
}
