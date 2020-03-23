use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use once_cell::sync::Lazy;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use thiserror::Error;
use url::Url;

static BASE_URL: Lazy<Url> = Lazy::new(|| Url::parse("https://codewars.com/api/v1/").unwrap());

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Error during HTTP handling")]
    Http(#[from] reqwest::Error),
    #[error("URL handling failed")]
    UrlParse(#[from] url::ParseError),
    #[error("Status code didn't indicate success (code {0})")]
    UnsuccessfulStatus(u16),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub username: String,
    pub name: Option<String>,
    pub honor: u32,
    pub clan: String,
    pub leaderboard_position: Option<u32>,
    pub skills: Option<HashSet<String>>,
    pub ranks: Ranks,
    pub code_challenges: CodeChallenges,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Ranks {
    pub overall: Language,
    pub languages: HashMap<String, Language>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Language {
    pub rank: i32,
    pub name: String,
    pub color: String,
    pub score: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeChallenges {
    pub total_authored: u32,
    pub total_completed: u32,
}

pub async fn user(username: &str) -> Result<User> {
    get_data(&format!("users/{}", username)).await
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletedChallenges {
    pub total_pages: u32,
    pub total_items: u32,
    pub data: Vec<CompletedChallenge>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletedChallenge {
    pub id: String,
    pub name: Option<String>,
    pub slug: Option<String>,
    pub completed_at: DateTime<Utc>,
    pub completed_languages: HashSet<String>,
}

pub async fn completed_challenges(username: &str) -> Result<CompletedChallenges> {
    get_data(&format!("users/{}/code-challenges/completed", username)).await
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthoredChallenges {
    pub data: Vec<AuthoredChallenge>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthoredChallenge {
    pub id: String,
    pub name: String,
    pub description: String,
    pub rank: i32,
    pub rank_name: String,
    pub tags: HashSet<String>,
    pub languages: HashSet<String>,
}

pub async fn authored_challenges(username: &str) -> Result<AuthoredChallenges> {
    get_data(&format!("users/{}/code-challenges/authored", username)).await
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeChallenge {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub category: String,
    pub published_at: DateTime<Utc>,
    pub approved_at: Option<DateTime<Utc>>,
    pub languages: HashSet<String>,
    pub url: Url,
    pub rank: Rank,
    pub created_by: ShortUser,
    pub approved_by: Option<ShortUser>,
    pub description: String,
    pub total_attempts: u32,
    pub total_completed: u32,
    pub total_stars: u32,
    pub tags: HashSet<String>,
    // Undocumented items
    pub contributors_wanted: bool,
    pub created_at: DateTime<Utc>,
    pub unresolved: Unresolved,
    pub vote_score: i32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rank {
    pub id: i32,
    pub name: String,
    pub color: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortUser {
    pub username: String,
    pub url: Url,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Unresolved {
    pub issues: u32,
    pub suggestions: u32,
}

pub async fn code_challenge(slug_or_id: &str) -> Result<CodeChallenge> {
    get_data(&format!("code-challenges/{}", slug_or_id)).await
}

async fn get_data<T: DeserializeOwned>(path: &str) -> Result<T> {
    let resp = reqwest::Client::new()
        .get(BASE_URL.join(path)?)
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(Error::UnsuccessfulStatus(resp.status().as_u16()));
    }

    Ok(resp.json().await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_lazy() {
        Lazy::force(&BASE_URL);
    }
}
