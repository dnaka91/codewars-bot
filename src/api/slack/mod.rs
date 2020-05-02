use thiserror::Error;

pub mod event;
pub mod webhook;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Error during HTTP handling")]
    Http(#[from] reqwest::Error),
    #[error("URL handling failed")]
    UrlParse(#[from] url::ParseError),
    #[error("Error reading environment variable")]
    EnvVar(#[from] std::env::VarError),
    #[error("Error during JSON (de-)serialization")]
    Json(#[from] serde_json::Error),
    #[error("Conversion from hex string failed")]
    Hex(#[from] hex::FromHexError),
    #[error("Failed sending a request to get {0}: {1}")]
    UnsuccessfulRequest(&'static str, String),
    #[error("Invalid HMAC key length")]
    HmacKeyLength,
    #[error("MAC verification error")]
    MacVerify,
    #[error("Missing `{0}` property in JSON object")]
    JsonMissingProperty(&'static str),
    #[error("JSON value `{0}` is not a {1}")]
    JsonWrongType(&'static str, &'static str),
    #[error("Unsupported signature version")]
    UnsupportedSignatureVersion,
}
