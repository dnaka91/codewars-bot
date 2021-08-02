//! Events that are sent from Slack to a server endpoint to notify about various changes in a team
//! chat.

use hmac::{Hmac, Mac, NewMac};
use serde::Deserialize;
use serde_json::Value;
use sha2::Sha256;

use super::{Error, Result};

/// An URL verification request that contains a challenge to be send back to Slack in a HTTP
/// response.
#[derive(Debug, Deserialize)]
pub struct UrlVerification {
    /// Message to send back.
    pub challenge: String,
}

/// An app mention event that happens when a user directly write a message to an app.
#[derive(Debug, Deserialize)]
pub struct AppMention {
    /// ID of the user who sent the message.
    pub user: String,
    /// Message content.
    pub text: String,
    /// The channel where this message was sent.
    pub channel: String,
}

/// Verify the signature of a HTTP request to make sure it really came from Slack. The system sends
/// a signature and timestamp with every request. The signature is a HMAC over the timestamp and
/// message payload with an apps private key.
pub fn verify_signature(key: &[u8], signature: &str, timestamp: &str, body: &[u8]) -> Result<()> {
    if !signature.starts_with("v0=") {
        return Err(Error::UnsupportedSignatureVersion);
    }

    let sig_data = hex::decode(&signature[3..])?;

    let mut mac = Hmac::<Sha256>::new_from_slice(key)?;

    mac.update(b"v0:");
    mac.update(timestamp.as_bytes());
    mac.update(b":");
    mac.update(body);

    mac.verify(&sig_data)?;

    Ok(())
}

/// Callback type for URL verification.
const CALLBACK_URL_VERIFICATION: &str = "url_verification";
/// Callback type for actual event messages.
const CALLBACK_EVENT_CALLBACK: &str = "event_callback";

/// Different kinds of callbacks that are sent by Slack.
pub enum Callback {
    /// Fallback for any callback types that are not supported.
    Unknown(String),
    /// URL verification that is used by Slack to make sure the service is running and can
    /// authenticate as the app registered in the platform.
    UrlVerification(UrlVerification),
    /// Callback for any kind of events that Slack might notify about.
    Event(Value),
}

/// Parse a JSON content into a Slack callback.
pub fn parse_callback(mut event: Value) -> Result<Callback> {
    Ok(
        match event
            .get("type")
            .ok_or(Error::JsonMissingProperty("type"))?
            .as_str()
            .ok_or(Error::JsonWrongType("type", "string"))?
        {
            CALLBACK_URL_VERIFICATION => {
                let event: UrlVerification = serde_json::from_value(event)?;
                Callback::UrlVerification(event)
            }
            CALLBACK_EVENT_CALLBACK => {
                let event = event
                    .get_mut("event")
                    .ok_or(Error::JsonMissingProperty("event"))?;

                Callback::Event(event.take())
            }
            callback_type => Callback::Unknown(callback_type.to_owned()),
        },
    )
}

/// Event type for any mentions of the app.
const EVENT_APP_MENTION: &str = "app_mention";

/// Different events that Slack can notify about.
pub enum Event {
    /// Fallback for any unsupported events.
    Unknown(String),
    /// The app was mentioned by a user directly like `@bot hello`.
    AppMention(AppMention),
}

/// Parse from raw JSON content into a Slack event.
#[allow(clippy::module_name_repetitions)]
pub fn parse_event(mut event: Value) -> Result<Event> {
    Ok(
        match event
            .as_object()
            .ok_or(Error::JsonWrongType("event", "object"))?
            .get("type")
            .ok_or(Error::JsonMissingProperty("type"))?
            .as_str()
            .ok_or(Error::JsonWrongType("type", "string"))?
        {
            EVENT_APP_MENTION => {
                let event: AppMention = serde_json::from_value(event.take())?;
                Event::AppMention(event)
            }
            event_type => Event::Unknown(event_type.to_owned()),
        },
    )
}
