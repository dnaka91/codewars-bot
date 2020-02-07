#![forbid(unsafe_code)]
#![deny(clippy::all, clippy::pedantic)]
#![warn(clippy::nursery)]

use anyhow::{Error, Result};
use futures::prelude::*;
use serde::de::DeserializeOwned;
use tokio_tungstenite::tungstenite::Message;

use crate::codewars::{AuthoredChallenges, CodeChallenge, CompletedChallenges, User, BASE_URL};

mod codewars;
mod slack;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv()?;

    println!(
        "{:#?}",
        tokio::try_join!(
            get_data::<User>("users/dnaka91"),
            get_data::<CompletedChallenges>("users/dnaka91/code-challenges/completed"),
            get_data::<AuthoredChallenges>("users/dnaka91/code-challenges/authored"),
            get_data::<CodeChallenge>("code-challenges/multiples-of-3-or-5")
        )?
    );

    let resp: slack::ConnectResponse = reqwest::Client::new()
        .post(slack::BASE_URL.join(slack::RTM_CONNECT)?)
        .form(&slack::RtmConnect {
            token: std::env::var("SLACK_TOKEN").unwrap(),
            ..slack::RtmConnect::default()
        })
        .send()
        .await?
        .json()
        .await?;

    println!("{:#?}", resp);

    let (ws, _) = tokio_tungstenite::connect_async(&resp.url).await?;
    let (mut write, read) = ws.split();

    write.send(Message::Pong(Vec::new())).await.unwrap();

    read.map_err(Error::from)
        .try_for_each(|message| async {
            match message {
                Message::Text(msg) => {
                    let message = serde_json::from_str::<slack::Event>(&msg)?;

                    println!("TEXT {:#?}", message);
                }
                Message::Binary(msg) => println!("BINARY {:?}", msg),
                Message::Ping(msg) => println!("PING {:?}", msg),
                Message::Pong(msg) => println!("PONG {:?}", msg),
                Message::Close(msg) => println!("CLOSE {:?}", msg),
            };
            Ok(())
        })
        .await?;

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
