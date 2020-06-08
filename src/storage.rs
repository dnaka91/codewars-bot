//! Storage for all bot related settings that are persisted as a single TOML file.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::Result;
use chrono::{NaiveTime, Weekday};
use serde::{Deserialize, Serialize};
use tokio::fs;

/// The repository is the single access point for all the **dynamic** settings regarding this bot.
/// Any changes to the settings through this repository are directly persisted to the TOML file.
///
/// Any manual changes to the file while the bot is running are not recognized and the application
/// must be restarted afterwards.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Repository {
    /// Location of the loaded repository.
    #[serde(skip)]
    path: PathBuf,
    /// List of users that are watched and used in any Codewars related actions.
    users: BTreeSet<String>,
    /// Whether to notify about any Codewars events related to the watched `users`.
    notify: bool,
    /// The schedule for weekly statistics messages.
    schedule: Schedule,
}

/// The schedule for weekly statistics reports.
#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Schedule {
    /// Day of the week when the reports should be send.
    pub weekday: Weekday,
    /// Exact time at the `weekday` when the reports should be send.
    pub time: NaiveTime,
}

impl Default for Schedule {
    fn default() -> Self {
        Self {
            weekday: Weekday::Sun,
            time: NaiveTime::from_hms(10, 0, 0),
        }
    }
}

impl Repository {
    /// Load all settings from the given file location. If the file doesn't exist, a new empty
    /// `Repository` with defaults is created instead.
    pub async fn load(path: impl AsRef<Path> + Send + Sync) -> Result<Self> {
        let mut repo = if path.as_ref().exists() {
            let settings = fs::read(&path).await?;
            toml::from_slice(&settings)?
        } else {
            Self::default()
        };

        repo.path = path.as_ref().to_owned();

        Ok(repo)
    }

    /// Persist the current settings to disk. The file location is the same where it was loaded
    /// from before.
    async fn save(&self) -> Result<()> {
        let settings = toml::to_string_pretty(self)?;

        fs::write(&self.path, &settings).await.map_err(Into::into)
    }

    /// Add a new user to the list of watched Codewars users. All commands that involve Codewars
    /// stats will include this new user in the queries. If the `username` was already in the list,
    /// nothing happens.
    pub async fn add_user(&mut self, username: &str) -> Result<bool> {
        if self.users.insert(username.to_owned()) {
            self.save().await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Remove a previously added user from the watchlist. If the `username` wasn't in the list,
    /// nothing happens.
    pub async fn remove_user(&mut self, username: &str) -> Result<bool> {
        if self.users.remove(username) {
            self.save().await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Create an iterator over all currently watched usernames. The iterator is distinct, so every
    /// username will only occur once.
    pub fn users(&self) -> impl Iterator<Item = &'_ str> {
        self.users.iter().map(String::as_str)
    }

    /// Get the current schedule for weekly Codewars statistics.
    pub const fn schedule(&self) -> &Schedule {
        &self.schedule
    }

    /// Set a new schedule for the weekly Codewars report.
    pub async fn set_schedule(&mut self, schedule: Schedule) -> Result<bool> {
        if self.schedule == schedule {
            Ok(false)
        } else {
            self.schedule = schedule;
            self.save().await?;
            Ok(true)
        }
    }

    /// Tell whether notifications about any new Codewars events for any watched user should be
    /// reported.
    pub const fn notify(&self) -> bool {
        self.notify
    }

    /// Set whether messages should be send for any new Codewars events of the watched users.
    pub async fn set_notify(&mut self, notify: bool) -> Result<bool> {
        if self.notify == notify {
            Ok(false)
        } else {
            self.notify = notify;
            self.save().await?;
            Ok(true)
        }
    }
}

#[cfg(test)]
mod tests {
    use tempfile::NamedTempFile;

    use super::*;

    #[tokio::test]
    async fn create() {
        let file = NamedTempFile::new().unwrap();
        let mut repo = Repository::load(file.path()).await.unwrap();
        repo.add_user("dnaka91").await.unwrap();
        repo.add_user("cschappert").await.unwrap();
        repo.remove_user("cschappert").await.unwrap();

        let mut users = repo.users();
        assert_eq!(users.next(), Some("dnaka91"));
        assert_eq!(users.next(), None);
    }
}
