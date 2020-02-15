use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::fs;

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Repository {
    #[serde(skip)]
    path: PathBuf,
    users: BTreeSet<String>,
}

impl Repository {
    pub async fn load(path: impl AsRef<Path>) -> Result<Self> {
        let mut repo = if path.as_ref().exists() {
            let settings = fs::read(&path).await?;
            toml::from_slice(&settings)?
        } else {
            Self::default()
        };

        repo.path = path.as_ref().to_owned();

        Ok(repo)
    }

    async fn save(&self) -> Result<()> {
        let settings = toml::to_string_pretty(self)?;

        fs::write(&self.path, &settings).await.map_err(Into::into)
    }

    pub async fn add_user(&mut self, username: &str) -> Result<bool> {
        if self.users.insert(username.to_owned()) {
            self.save().await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub async fn remove_user(&mut self, username: &str) -> Result<bool> {
        if self.users.remove(username) {
            self.save().await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn users(&self) -> impl Iterator<Item = &'_ str> {
        self.users.iter().map(String::as_str)
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
