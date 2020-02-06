#![forbid(unsafe_code)]
#![deny(clippy::all, clippy::pedantic)]
#![warn(clippy::nursery)]

use anyhow::Result;
use serde::de::DeserializeOwned;

use crate::codewars::{AuthoredChallenges, CodeChallenge, CompletedChallenges, User, BASE_URL};

mod codewars;

#[tokio::main]
async fn main() -> Result<()> {
    println!(
        "{:#?}",
        tokio::try_join!(
            get_data::<User>("users/dnaka91"),
            get_data::<CompletedChallenges>("users/dnaka91/code-challenges/completed"),
            get_data::<AuthoredChallenges>("users/dnaka91/code-challenges/authored"),
            get_data::<CodeChallenge>("code-challenges/multiples-of-3-or-5")
        )?
    );
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
