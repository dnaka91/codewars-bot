use once_cell::sync::Lazy;
use serde::Serialize;
use thiserror::Error;
use url::Url;

pub mod event;
pub mod rtm;
pub mod web;
pub mod webhook;

static BASE_URL: Lazy<Url> = Lazy::new(|| Url::parse("https://slack.com/api/").unwrap());

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Error during HTTP handling")]
    Http(#[from] reqwest::Error),
    #[error("URL handling failed")]
    UrlParse(#[from] url::ParseError),
    #[error("Error reading environment variable")]
    EnvVar(#[from] std::env::VarError),
    #[error("Error during WebSocket connection")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),
    #[error("Error during JSON (de-)serialization")]
    Json(#[from] serde_json::Error),
    #[error("Failed sending a request to get {0}: {1}")]
    UnsuccessfulRequest(&'static str, String),
    #[error("Status code didn't indicate success (code {0})")]
    UnsuccessfulStatus(u16),
    #[error("Response JSON is not in the expected format")]
    InvalidJson,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum Block<'a> {
    Divider,
    Section { text: Element<'a> },
    Context { elements: &'a [Element<'a>] },
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum Element<'a> {
    Mrkdwn { text: &'a str },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_lazy() {
        Lazy::force(&BASE_URL);
    }

    #[test]
    fn message_payload() {
        let json = serde_json::to_string_pretty(&[
            Block::Divider,
            Block::Section {
                text: Element::Mrkdwn { text: "hello" },
            },
            Block::Context {
                elements: &[Element::Mrkdwn {
                    text: "*Author:* me",
                }],
            },
        ])
        .unwrap();
        println!("{}", json);
    }
}
