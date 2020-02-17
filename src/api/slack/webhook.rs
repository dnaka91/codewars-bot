use reqwest::IntoUrl;
use serde::Serialize;

use super::{Block, Error, Result};

#[derive(Debug, Serialize)]
pub struct Message<'a> {
    pub text: &'a str,
    pub blocks: Option<&'a [&'a Block<'a>]>,
}

pub async fn send<U: IntoUrl>(url: U, text: &str) -> Result<()> {
    let resp = reqwest::Client::new()
        .post(url)
        .json(&Message { text, blocks: None })
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(Error::UnsuccessfulRequest(
            "webhook",
            "Failed posting to webhook".to_owned(),
        ));
    }

    Ok(())
}
