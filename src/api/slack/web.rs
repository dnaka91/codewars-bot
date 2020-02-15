use std::env;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use url::Url;

use super::{Block, Error, Result, BASE_URL};

const RTM_CONNECT: &str = "rtm.connect";
const USERS_CONVERSATIONS: &str = "users.conversations";
const CHAT_POST_MESSAGE: &str = "chat.postMessage";
const USERS_LIST: &str = "users.list";

#[derive(Debug, Deserialize)]
struct BasicResponse {
    ok: bool,
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

#[derive(Debug, Deserialize)]
pub struct ChatPostMessageResponse {
    pub ok: bool,
}

pub async fn chat_post_message(channel: &str, text: &str) -> Result<()> {
    send_request::<BasicResponse>(
        CHAT_POST_MESSAGE,
        reqwest::Client::new()
            .post(BASE_URL.join(CHAT_POST_MESSAGE)?)
            .form(&ChatPostMessageRequest {
                token: &env::var("SLACK_TOKEN")?,
                channel,
                text,
                blocks: None,
                icon_emoji: Some(":crossed_swords:"),
                username: Some("Codewars Bot"),
            }),
    )
    .await?;

    Ok(())
}

#[derive(Debug, Serialize)]
pub struct RtmConnectRequest<'a> {
    pub token: &'a str,
}

#[derive(Debug, Deserialize)]
pub struct RtmConnectResponse {
    pub url: Url,
}

pub async fn rtm_connect() -> Result<Url> {
    let resp: RtmConnectResponse = send_request(
        RTM_CONNECT,
        reqwest::Client::new()
            .post(BASE_URL.join(RTM_CONNECT)?)
            .form(&RtmConnectRequest {
                token: &env::var("SLACK_BOT_TOKEN")?,
            }),
    )
    .await?;

    Ok(resp.url)
}

#[derive(Debug, Serialize)]
pub struct UsersConversationsRequest<'a> {
    pub token: &'a str,
}

#[derive(Debug, Deserialize)]
pub struct UsersConversationsResponse {
    pub channels: Vec<Channel>,
}

#[derive(Debug, Deserialize)]
pub struct Channel {
    pub id: String,
    pub name: String,
    pub is_channel: bool,
    pub is_archived: bool,
}

pub async fn users_conversations() -> Result<Vec<Channel>> {
    let resp: UsersConversationsResponse = send_request(
        USERS_CONVERSATIONS,
        reqwest::Client::new()
            .post(BASE_URL.join(USERS_CONVERSATIONS)?)
            .form(&UsersConversationsRequest {
                token: &env::var("SLACK_TOKEN")?,
            }),
    )
    .await?;

    Ok(resp.channels)
}

#[derive(Debug, Serialize)]
pub struct UsersListRequest<'a> {
    pub token: &'a str,
}

#[derive(Debug, Deserialize)]
pub struct UsersListResponse {
    pub members: Vec<User>,
}

#[derive(Debug, Deserialize)]
pub struct User {
    pub id: String,
    pub deleted: bool,
    pub name: String,
    pub is_bot: bool,
}

pub async fn users_list() -> Result<Vec<User>> {
    let resp: UsersListResponse = send_request(
        USERS_LIST,
        reqwest::Client::new()
            .post(BASE_URL.join(USERS_LIST)?)
            .form(&UsersListRequest {
                token: &env::var("SLACK_BOT_TOKEN")?,
            }),
    )
    .await?;

    Ok(resp.members)
}

#[derive(Debug, Deserialize)]
struct ErrorResponse {
    error: String,
}

async fn send_request<T>(method: &'static str, builder: reqwest::RequestBuilder) -> Result<T>
where
    T: DeserializeOwned,
{
    let resp = builder.send().await?;

    if !resp.status().is_success() {
        return Err(Error::UnsuccessfulStatus(resp.status().as_u16()));
    }

    let resp: Value = resp.json().await?;
    let object = resp.as_object().ok_or_else(|| Error::InvalidJson)?;
    let ok = object
        .get("ok")
        .ok_or_else(|| Error::InvalidJson)?
        .as_bool()
        .ok_or_else(|| Error::InvalidJson)?;

    if !ok {
        return Err(Error::UnsuccessfulRequest(
            method,
            serde_json::from_value::<ErrorResponse>(resp)?.error,
        ));
    }

    Ok(serde_json::from_value(resp)?)
}
