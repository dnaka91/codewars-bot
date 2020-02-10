#![forbid(unsafe_code)]
#![deny(clippy::all, clippy::pedantic)]
#![warn(clippy::nursery)]

use std::env;

use anyhow::Result;
use futures::prelude::*;
use log::{debug, info};
use serde::de::DeserializeOwned;

use crate::codewars::{AuthoredChallenges, CodeChallenge, CompletedChallenges, User, BASE_URL};

mod codewars;
mod slack;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv()?;
    pretty_env_logger::try_init()?;

    debug!(
        "{:#?}",
        tokio::try_join!(
            get_data::<User>("users/dnaka91"),
            get_data::<CompletedChallenges>("users/dnaka91/code-challenges/completed"),
            get_data::<AuthoredChallenges>("users/dnaka91/code-challenges/authored"),
            get_data::<CodeChallenge>("code-challenges/multiples-of-3-or-5")
        )?
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

async fn get_data<T: DeserializeOwned>(path: &str) -> Result<T> {
    Ok(reqwest::Client::new()
        .get(BASE_URL.join(path)?)
        .send()
        .await?
        .json()
        .await?)
}
