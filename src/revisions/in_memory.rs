use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::common::{Configurable, RevisionToken, VaultPath};
use crate::revisions::RevisionTracker;
use crate::revisions::config::{InMemoryRevisionTrackerConfig, RevisionTrackerConfig};
use crate::revisions::errors::RevisionTrackerError;

const REVISIONS_PERSIST_VERSION: u32 = 1;
const REVISIONS_FILE: &str = "revisions.json";

#[derive(Deserialize)]
struct RevisionsFile {
    version: u32,
    revisions: HashMap<VaultPath, RevisionToken>,
}

#[derive(Serialize)]
struct RevisionsFileRef<'a> {
    version: u32,
    revisions: &'a HashMap<VaultPath, RevisionToken>,
}

pub struct InMemoryRevisionTracker {
    revisions: RwLock<HashMap<VaultPath, RevisionToken>>,
    persistence_path: Option<PathBuf>,
}

impl InMemoryRevisionTracker {
    pub fn new(persistence_path: Option<PathBuf>) -> Result<Self, RevisionTrackerError> {
        let mut revisions = HashMap::new();

        if let Some(ref dir) = persistence_path {
            let file_path = dir.join(REVISIONS_FILE);
            if file_path.exists() {
                let bytes = std::fs::read(&file_path)?;
                match serde_json::from_slice::<RevisionsFile>(&bytes) {
                    Ok(file) if file.version == REVISIONS_PERSIST_VERSION => {
                        revisions = file.revisions;
                    }
                    Ok(_) => {
                        // Version mismatch — start fresh
                    }
                    Err(_) => {
                        // Corrupt file — start fresh
                    }
                }
            }
        }

        Ok(Self {
            revisions: RwLock::new(revisions),
            persistence_path,
        })
    }

    async fn persist(&self) {
        let Some(ref dir) = self.persistence_path else {
            return;
        };

        if let Err(e) = std::fs::create_dir_all(dir) {
            tracing::warn!(path = %dir.display(), error = %e, "failed to create revisions directory");
            return;
        }

        let guard = self.revisions.read().await;
        let file = RevisionsFileRef {
            version: REVISIONS_PERSIST_VERSION,
            revisions: &guard,
        };
        let bytes = match serde_json::to_vec(&file) {
            Ok(bytes) => bytes,
            Err(e) => {
                tracing::warn!(error = %e, "failed to serialize revisions");
                return;
            }
        };
        drop(guard);

        let file_path = dir.join(REVISIONS_FILE);
        if let Err(e) = std::fs::write(&file_path, &bytes) {
            tracing::warn!(path = %file_path.display(), error = %e, "failed to persist revisions");
        }
    }
}

impl Configurable for InMemoryRevisionTracker {
    type Config = RevisionTrackerConfig;

    fn config(&self) -> Self::Config {
        RevisionTrackerConfig::InMemory(InMemoryRevisionTrackerConfig {
            persistence_path: self.persistence_path.clone(),
        })
    }
}

impl RevisionTracker for InMemoryRevisionTracker {
    async fn get_revision(&self, path: &VaultPath) -> Option<RevisionToken> {
        self.revisions.read().await.get(path).cloned()
    }

    async fn update_revision(&self, path: &VaultPath, token: RevisionToken) {
        self.revisions.write().await.insert(path.clone(), token);
        self.persist().await;
    }

    async fn remove_revision(&self, path: &VaultPath) {
        self.revisions.write().await.remove(path);
        self.persist().await;
    }

    async fn validate_revision(&self, path: &VaultPath, token: &RevisionToken) -> bool {
        match self.revisions.read().await.get(path) {
            None => true,
            Some(stored) => stored == token,
        }
    }

    async fn all_revisions(&self) -> Vec<(VaultPath, RevisionToken)> {
        self.revisions
            .read()
            .await
            .iter()
            .map(|(p, t)| (p.clone(), t.clone()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn get_revision_unknown_path() {
        let tracker = InMemoryRevisionTracker::new(None).unwrap();
        let path: VaultPath = "note.md".try_into().unwrap();
        assert!(tracker.get_revision(&path).await.is_none());
    }

    #[tokio::test]
    async fn update_and_get_roundtrip() {
        let tracker = InMemoryRevisionTracker::new(None).unwrap();
        let path: VaultPath = "note.md".try_into().unwrap();
        let token = RevisionToken::from("abc123");

        tracker.update_revision(&path, token.clone()).await;
        assert_eq!(tracker.get_revision(&path).await, Some(token));
    }

    #[tokio::test]
    async fn remove_revision_removes_entry() {
        let tracker = InMemoryRevisionTracker::new(None).unwrap();
        let path: VaultPath = "note.md".try_into().unwrap();
        tracker
            .update_revision(&path, RevisionToken::from("abc"))
            .await;

        tracker.remove_revision(&path).await;
        assert!(tracker.get_revision(&path).await.is_none());
    }

    #[tokio::test]
    async fn validate_matching_token() {
        let tracker = InMemoryRevisionTracker::new(None).unwrap();
        let path: VaultPath = "note.md".try_into().unwrap();
        let token = RevisionToken::from("abc");
        tracker.update_revision(&path, token.clone()).await;

        assert!(tracker.validate_revision(&path, &token).await);
    }

    #[tokio::test]
    async fn validate_mismatched_token() {
        let tracker = InMemoryRevisionTracker::new(None).unwrap();
        let path: VaultPath = "note.md".try_into().unwrap();
        tracker
            .update_revision(&path, RevisionToken::from("abc"))
            .await;

        let stale = RevisionToken::from("old");
        assert!(!tracker.validate_revision(&path, &stale).await);
    }

    #[tokio::test]
    async fn validate_untracked_path_returns_true() {
        let tracker = InMemoryRevisionTracker::new(None).unwrap();
        let path: VaultPath = "unknown.md".try_into().unwrap();
        let token = RevisionToken::from("any");
        assert!(tracker.validate_revision(&path, &token).await);
    }

    #[tokio::test]
    async fn all_revisions_returns_all_entries() {
        let tracker = InMemoryRevisionTracker::new(None).unwrap();
        let p1: VaultPath = "a.md".try_into().unwrap();
        let p2: VaultPath = "b.md".try_into().unwrap();
        tracker
            .update_revision(&p1, RevisionToken::from("t1"))
            .await;
        tracker
            .update_revision(&p2, RevisionToken::from("t2"))
            .await;

        let all = tracker.all_revisions().await;
        assert_eq!(all.len(), 2);
    }

    #[tokio::test]
    async fn persistence_roundtrip() {
        let dir = TempDir::new().unwrap();
        let path: VaultPath = "note.md".try_into().unwrap();
        let token = RevisionToken::from("rev1");

        {
            let tracker = InMemoryRevisionTracker::new(Some(dir.path().to_path_buf())).unwrap();
            tracker.update_revision(&path, token.clone()).await;
        }

        // Recreate from same path — should load persisted data
        let tracker = InMemoryRevisionTracker::new(Some(dir.path().to_path_buf())).unwrap();
        assert_eq!(tracker.get_revision(&path).await, Some(token));
    }

    #[tokio::test]
    async fn persistence_remove_persists() {
        let dir = TempDir::new().unwrap();
        let path: VaultPath = "note.md".try_into().unwrap();

        {
            let tracker = InMemoryRevisionTracker::new(Some(dir.path().to_path_buf())).unwrap();
            tracker
                .update_revision(&path, RevisionToken::from("rev1"))
                .await;
            tracker.remove_revision(&path).await;
        }

        let tracker = InMemoryRevisionTracker::new(Some(dir.path().to_path_buf())).unwrap();
        assert!(tracker.get_revision(&path).await.is_none());
    }
}
