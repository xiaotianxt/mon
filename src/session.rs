use std::fs;
use std::path::Path;

use anyhow::Context;
use anyhow::Result;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredSession {
    pub token: String,
    pub created_at: String,
}

pub fn save(path: &Path, token: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let stored = StoredSession {
        token: token.to_owned(),
        created_at: Utc::now().to_rfc3339(),
    };
    let bytes = serde_json::to_vec_pretty(&stored).context("failed to encode session")?;
    fs::write(path, bytes).with_context(|| format!("failed to write {}", path.display()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            .with_context(|| format!("failed to chmod {}", path.display()))?;
    }

    Ok(())
}

pub fn load(path: &Path) -> Result<StoredSession> {
    let bytes = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    let stored: StoredSession = serde_json::from_slice(&bytes)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    if stored.token.trim().is_empty() {
        anyhow::bail!("session token is empty");
    }
    Ok(stored)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saves_and_loads_session_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.json");
        save(&path, "abc123").unwrap();
        let loaded = load(&path).unwrap();
        assert_eq!(loaded.token, "abc123");
    }
}
