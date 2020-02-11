#![forbid(unsafe_code)]
#![deny(clippy::all, clippy::pedantic)]
#![warn(clippy::nursery)]

use std::env;

use anyhow::Result;
use futures::prelude::*;
use log::{debug, info};

mod codewars;
mod slack;

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

    let (_, mut r) = slack::rtm_connect().await?;
    let target_channel = env::var("SLACK_CHANNEL")?;

    while let Some(event) = r.next().await {
        info!("EVENT {:?}", event);

        if let slack::Event::Message {
            channel,
            user,
            text,
            ..
        } = event
        {
            if channel == target_channel && text.starts_with("!codewars-bot") {
                slack::chat_post_message(&channel, &format!("<@{}> Hey there", user)).await?;
            }
        }
    }

    Ok(())
}
