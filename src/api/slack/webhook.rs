use reqwest::IntoUrl;
use serde::Serialize;

use super::{Error, Result};

#[derive(Debug, Serialize)]
pub struct Message<'a> {
    pub text: &'a str,
}

pub async fn send<U: IntoUrl + Send>(url: U, text: &str) -> Result<()> {
    let resp = reqwest::Client::new()
        .post(url)
        .json(&Message { text })
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
