//! Global server settings loaded at start up and used to configure the service and provide required
//! information for its functionality.

use std::fs;

use anyhow::{bail, Result};
use serde::Deserialize;

/// All settings that are loaded at start up and required by the service to function.
#[derive(Deserialize)]
pub struct Settings {
    /// Port to listen for connections. Defaults to `8080` if not set.
    #[serde(default = "default_port")]
    pub port: u16,
    /// Signing key to verify HTTP calls come from Slack.
    pub signing_key: String,
    /// Webhook URL to post messages to a Slack channel.
    pub webhook_url: String,
}

/// Default value for the port.
const fn default_port() -> u16 {
    8080
}

/// Load the settings from a TOML file in several common known locations.
pub fn load() -> Result<Settings> {
    let locations = &[
        concat!("/etc/", env!("CARGO_PKG_NAME"), "/config.toml"),
        concat!("/app/", env!("CARGO_PKG_NAME"), ".toml"),
        concat!(env!("CARGO_PKG_NAME"), ".toml"),
    ];
    let buf = locations.iter().find_map(|loc| fs::read(loc).ok());

    match buf {
        Some(buf) => Ok(toml::from_slice(&buf)?),
        None => bail!("failed finding settings"),
    }
}
