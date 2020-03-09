use anyhow::{anyhow, ensure, Result};
use hmac::{Hmac, Mac};
use serde::Deserialize;
use serde_json::Value;
use sha2::Sha256;

#[derive(Debug, Deserialize)]
pub struct UrlVerification {
    pub challenge: String,
}

#[derive(Debug, Deserialize)]
pub struct AppMention {
    pub user: String,
    pub text: String,
    pub channel: String,
}

pub fn verify_signature(key: &[u8], signature: &str, timestamp: &str, body: &[u8]) -> Result<()> {
    ensure!(
        signature.starts_with("v0="),
        "unsupported signature version"
    );

    let sig_data = hex::decode(&signature[3..])?;

    let mut mac = Hmac::<Sha256>::new_varkey(key).map_err(|_| anyhow!("Invalid key size"))?;

    mac.input(b"v0:");
    mac.input(timestamp.as_bytes());
    mac.input(b":");
    mac.input(body);

    mac.verify(&sig_data).map_err(|e| anyhow!(e.to_string()))?;

    Ok(())
}

const CALLBACK_URL_VERIFICATION: &str = "url_verification";
const CALLBACK_EVENT_CALLBACK: &str = "event_callback";

pub enum Callback {
    Unknown(String),
    UrlVerification(UrlVerification),
    Event(Value),
}

pub fn parse_callback(mut event: Value) -> Result<Callback> {
    Ok(
        match event
            .get("type")
            .ok_or_else(|| anyhow!("missing `type` property"))?
            .as_str()
            .ok_or_else(|| anyhow!("type is not a string"))?
        {
            CALLBACK_URL_VERIFICATION => {
                let event: UrlVerification = serde_json::from_value(event)?;
                Callback::UrlVerification(event)
            }
            CALLBACK_EVENT_CALLBACK => {
                let event = event
                    .get_mut("event")
                    .ok_or_else(|| anyhow!("missing `event` property"))?;

                Callback::Event(event.take())
            }
            callback_type => Callback::Unknown(callback_type.to_owned()),
        },
    )
}

const EVENT_APP_MENTION: &str = "app_mention";

pub enum Event {
    Unknown(String),
    AppMention(AppMention),
}

#[allow(clippy::module_name_repetitions)]
pub fn parse_event(mut event: Value) -> Result<Event> {
    Ok(
        match event
            .as_object()
            .ok_or_else(|| anyhow!("event is not an object"))?
            .get("type")
            .ok_or_else(|| anyhow!("missing `type` property"))?
            .as_str()
            .ok_or_else(|| anyhow!("type is not a string"))?
        {
            EVENT_APP_MENTION => {
                let event: AppMention = serde_json::from_value(event.take())?;
                Event::AppMention(event)
            }
            event_type => Event::Unknown(event_type.to_owned()),
        },
    )
}
