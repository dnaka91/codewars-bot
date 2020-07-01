//! Functions for sending messages via web hooks.

use reqwest::IntoUrl;
use serde::Serialize;

use super::{Error, Result};

/// The representation of a Slack message in it's simplest form with only the text content.
#[derive(Debug, Serialize)]
pub struct Message<'a> {
    /// Text content of the message.
    pub text: &'a str,
}

/// Send given message to a web hook URL. The message can be plain text but also Slack style
/// Markdown content.
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
