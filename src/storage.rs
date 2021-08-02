//! Storage for all bot related settings that are persisted as a single TOML file.

use std::{collections::BTreeSet, path::Path};

use anyhow::Result;
use chrono::prelude::*;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use tokio::{fs, sync::Mutex};

const STATE_DIR: &str = concat!("/var/lib/", env!("CARGO_PKG_NAME"));
const STATE_FILE: &str = concat!("/var/lib/", env!("CARGO_PKG_NAME"), "/state.toml");
const TEMP_FILE: &str = concat!("/var/lib/", env!("CARGO_PKG_NAME"), "/~temp-state.toml");

/// The repository is the single access point for all the **dynamic** settings regarding this bot.
/// Any changes to the settings through this repository are directly persisted to the TOML file.
///
/// Any manual changes to the file while the bot is running are not recognized and the application
/// must be restarted afterwards.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Repository {
    /// List of users that are watched and used in any Codewars related actions.
    users: BTreeSet<String>,
    /// Whether to notify about any Codewars events related to the watched `users`.
    notify: bool,
    /// Last time the schedule was successfully sent.
    last_run: Option<DateTime<Utc>>,
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
    pub async fn load() -> Result<Self> {
        let repo = if Path::new(STATE_FILE).exists() {
            let settings = fs::read(STATE_FILE).await?;
            toml::from_slice(&settings)?
        } else {
            Self::default()
        };

        Ok(repo)
    }

    /// Persist the current settings to disk. The file location is the same where it was loaded
    /// from before.
    async fn save(&self) -> Result<()> {
        static LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

        let _guard = LOCK.lock().await;

        fs::create_dir_all(STATE_DIR).await?;

        let settings = toml::to_string_pretty(self)?;

        fs::write(TEMP_FILE, &settings).await?;
        fs::rename(TEMP_FILE, STATE_FILE).await?;

        Ok(())
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

    /// Get the time of the last scheduled stats run.
    pub const fn last_run(&self) -> Option<DateTime<Utc>> {
        self.last_run
    }

    /// Set the last run of scheduled stats.
    pub async fn set_last_run(&mut self, last_run: DateTime<Utc>) -> Result<bool> {
        if self.last_run == Some(last_run) {
            Ok(false)
        } else {
            self.last_run = Some(last_run);
            self.save().await?;
            Ok(true)
        }
    }
}
