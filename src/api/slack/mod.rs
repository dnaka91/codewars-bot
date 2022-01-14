//! Slack API for parsing events received from the platform and webhooks to send messages.

use thiserror::Error;

pub mod event;
pub mod webhook;

/// Shorthand for results in this module.
pub type Result<T> = std::result::Result<T, Error>;

/// A list of errors that can happen while interacting with the API.
#[derive(Debug, Error)]
pub enum Error {
    #[error("Error during HTTP handling")]
    Http(#[from] reqwest::Error),
    #[error("URL handling failed")]
    UrlParse(#[from] url::ParseError),
    #[error("Error during JSON (de-)serialization")]
    Json(#[from] serde_json::Error),
    #[error("Conversion from hex string failed")]
    Hex(#[from] hex::FromHexError),
    #[error("Failed sending a request to get {0}: {1}")]
    UnsuccessfulRequest(&'static str, String),
    #[error("Invalid HMAC key length")]
    HmacKeyLength(#[from] hmac::digest::crypto_common::InvalidLength),
    #[error("MAC verification error")]
    MacVerify(#[from] hmac::digest::MacError),
    #[error("Missing `{0}` property in JSON object")]
    JsonMissingProperty(&'static str),
    #[error("JSON value `{0}` is not a {1}")]
    JsonWrongType(&'static str, &'static str),
    #[error("Unsupported signature version")]
    UnsupportedSignatureVersion,
}
