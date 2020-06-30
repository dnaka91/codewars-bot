use anyhow::Result;
use config::{Config, File};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Settings {
    /// Port to listen for connections.
    #[serde(default = "default_port")]
    pub port: u16,
    /// Signing key to verify HTTP calls come from Slack.
    pub signing_key: String,
    /// Webhook URL to post messages to a Slack channel.
    pub webhook_url: String,
}

const fn default_port() -> u16 {
    8080
}

pub fn load() -> Result<Settings> {
    let mut s = Config::new();

    s.merge(File::with_name("/app/codewars-bot.toml").required(false))?;
    s.merge(File::with_name("codewars-bot.toml").required(false))?;

    s.try_into().map_err(Into::into)
}
