use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use once_cell::sync::Lazy;
use serde::Deserialize;
use url::Url;

pub static BASE_URL: Lazy<Url> = Lazy::new(|| Url::parse("https://codewars.com/api/v1/").unwrap());

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub username: String,
    pub name: String,
    pub honor: u32,
    pub clan: String,
    pub leaderboard_position: Option<u32>,
    pub skills: HashSet<String>,
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
    pub name: String,
    pub slug: String,
    pub completed_at: DateTime<Utc>,
    pub completed_languages: HashSet<String>,
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
