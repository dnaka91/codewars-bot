use std::env;

use anyhow::{ensure, Result};
use futures::channel::mpsc::{self, UnboundedReceiver, UnboundedSender};
use futures::prelude::*;
use log::trace;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio_tungstenite::tungstenite::Message;
use url::Url;

pub static BASE_URL: Lazy<Url> = Lazy::new(|| Url::parse("https://slack.com/api/").unwrap());

pub const RTM_CONNECT: &str = "rtm.connect";
pub const USERS_CONVERSATIONS: &str = "users.conversations";
pub const CHAT_POST_MESSAGE: &str = "chat.postMessage";
pub const USERS_LIST: &str = "users.list";

#[derive(Debug, Serialize)]
pub struct RtmConnectRequest<'a> {
    pub token: &'a str,
}

#[derive(Debug, Deserialize)]
pub struct RtmConnectResponse {
    pub ok: bool,
    pub url: Url,
}

#[derive(Debug, Serialize)]
pub struct UsersConversationsRequest<'a> {
    pub token: &'a str,
}

#[derive(Debug, Deserialize)]
pub struct UsersConversationsResponse {
    pub ok: bool,
    pub channels: Option<Vec<Channel>>,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Channel {
    pub id: String,
    pub name: String,
    pub is_channel: bool,
    pub is_archived: bool,
}

#[derive(Debug, Serialize)]
pub struct ChatPostMessageRequest<'a> {
    pub token: &'a str,
    pub channel: &'a str,
    pub text: &'a str,
    pub blocks: Option<&'a [Block<'a>]>,
    pub icon_emoji: Option<&'a str>,
    pub username: Option<&'a str>,
}

#[derive(Debug, Serialize)]
pub struct UsersListRequest<'a> {
    pub token: &'a str,
}

#[derive(Debug, Deserialize)]
pub struct UsersListResponse {
    pub ok: bool,
    pub members: Option<Vec<User>>,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct User {
    pub id: String,
    pub deleted: bool,
    pub name: String,
    pub is_bot: bool,
}
#[derive(Debug, Deserialize)]
pub struct ChatPostMessageResponse {
    pub ok: bool,
    pub error: Option<String>,
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

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum Event {
    Hello,
    Error {
        code: i32,
        msg: String,
    },
    Message {
        channel: String,
        user: String,
        text: String,
        ts: String,
    },
}

#[derive(Debug, Serialize)]
pub struct WebhookMessage<'a> {
    pub text: &'a str,
    pub blocks: Option<&'a [&'a Block<'a>]>,
}

pub async fn users_conversations() -> Result<Vec<Channel>> {
    let resp: UsersConversationsResponse = reqwest::Client::new()
        .post(BASE_URL.join(USERS_CONVERSATIONS)?)
        .form(&UsersConversationsRequest {
            token: &env::var("SLACK_TOKEN")?,
        })
        .send()
        .await?
        .json()
        .await?;

    ensure!(
        resp.ok,
        "failed reading user's conversation list {:?}",
        resp.error
    );
    Ok(resp.channels.unwrap())
}

pub async fn users_list() -> Result<Vec<User>> {
    let resp: UsersListResponse = reqwest::Client::new()
        .post(BASE_URL.join(USERS_LIST)?)
        .form(&UsersListRequest {
            token: &env::var("SLACK_BOT_TOKEN")?,
        })
        .send()
        .await?
        .json()
        .await?;

    ensure!(resp.ok, "failed reading user list {:?}", resp.error);
    Ok(resp.members.unwrap())
}

pub async fn chat_post_message(channel: &str, text: &str) -> Result<()> {
    let resp: ChatPostMessageResponse = reqwest::Client::new()
        .post(BASE_URL.join(CHAT_POST_MESSAGE)?)
        .form(&ChatPostMessageRequest {
            token: &env::var("SLACK_TOKEN")?,
            channel,
            text,
            blocks: None,
            icon_emoji: Some(":crossed_swords:"),
            username: Some("Codewars Bot"),
        })
        .send()
        .await?
        .json()
        .await?;

    ensure!(resp.ok, "failed posting chat message {:?}", resp.error);
    Ok(())
}

pub async fn webhook_message(text: &str) -> Result<()> {
    let resp = reqwest::Client::new()
        .post(&env::var("SLACK_WEBHOOK")?)
        .json(&WebhookMessage { text, blocks: None })
        .send()
        .await?;

    ensure!(resp.status().is_success(), "failed posting to webhook");
    Ok(())
}

pub async fn rtm_connect() -> Result<(UnboundedSender<Value>, UnboundedReceiver<Event>)> {
    let resp: RtmConnectResponse = reqwest::Client::new()
        .post(BASE_URL.join(RTM_CONNECT)?)
        .form(&RtmConnectRequest {
            token: &env::var("SLACK_BOT_TOKEN")?,
        })
        .send()
        .await?
        .json()
        .await?;

    let (ws, _) = tokio_tungstenite::connect_async(&resp.url).await?;
    let (write, mut read) = ws.split();

    let (tx, rx) = mpsc::unbounded();

    tokio::spawn(rx.map(Ok).forward(write));

    let (value_tx, value_rx) = mpsc::unbounded();
    let (event_tx, event_rx) = mpsc::unbounded();

    tokio::spawn(
        value_rx
            .map(|v: Value| Message::Text(serde_json::to_string(&v).unwrap()))
            .map(Ok)
            .forward(tx.clone()),
    );

    tokio::spawn(async move {
        while let Some(message) = read.try_next().await.unwrap() {
            match message {
                Message::Text(msg) => {
                    let message = serde_json::from_str::<Value>(&msg).unwrap();
                    let raw_msg = message.as_object().unwrap();
                    let msg_type = raw_msg.get("type").unwrap().as_str().unwrap();
                    let types = &["hello", "error", "message"];

                    if types.contains(&msg_type) && message.get("subtype").is_none() {
                        let event = serde_json::from_value::<Event>(message).unwrap();
                        trace!("TEXT {:?}", event);
                        event_tx.unbounded_send(event).unwrap();
                    } else {
                        trace!(
                            "unsupported event `{}` (subtype {:?})",
                            msg_type,
                            message.get("subtype").map(Value::as_str).flatten()
                        );
                    }
                }
                Message::Binary(msg) => trace!("BINARY {:?}", msg),
                Message::Ping(msg) => {
                    trace!("PING {:?}", msg);
                    tx.unbounded_send(Message::Pong(msg)).unwrap();
                }
                Message::Pong(msg) => trace!("PONG {:?}", msg),
                Message::Close(msg) => trace!("CLOSE {:?}", msg),
            }
        }
    });

    Ok((value_tx, event_rx))
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
