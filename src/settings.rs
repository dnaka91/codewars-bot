//! Global server settings loaded at start up and used to configure the service and provide required
//! information for its functionality.

use anyhow::Result;
use config::{Config, File};
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
    let mut s = Config::new();

    s.merge(File::with_name("/app/codewars-bot.toml").required(false))?;
    s.merge(File::with_name("codewars-bot.toml").required(false))?;

    s.try_into().map_err(Into::into)
}
