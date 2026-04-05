use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::error::{AppError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockFile {
    pub version: u32,
    pub skills: HashMap<String, LockedEntry>,
    #[serde(default)]
    pub dismissed: HashMap<String, bool>,
    #[serde(rename = "lastSelectedAgents", default)]
    pub last_selected_agents: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LockedEntry {
    pub source: String,
    pub source_type: String,
    pub source_url: String,
    pub skill_path: String,
    pub skill_folder_hash: String,
    pub installed_at: String,
    pub updated_at: String,
}

impl LockFile {
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::empty());
        }
        let data = std::fs::read_to_string(path)?;
        let lock: Self = serde_json::from_str(&data)?;
        Ok(lock)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let data = serde_json::to_string_pretty(self)?;
        std::fs::write(path, data)?;
        Ok(())
    }

    pub fn empty() -> Self {
        Self {
            version: 3,
            skills: HashMap::new(),
            dismissed: HashMap::new(),
            last_selected_agents: Vec::new(),
        }
    }

    pub fn add_entry(&mut self, name: String, entry: LockedEntry) {
        self.skills.insert(name, entry);
    }

    pub fn remove_entry(&mut self, name: &str) -> Option<LockedEntry> {
        self.skills.remove(name)
    }
}

impl LockedEntry {
    pub fn new_github(source: &str, skill_path: &str, hash: &str) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            source: source.to_string(),
            source_type: "github".to_string(),
            source_url: format!("https://github.com/{source}.git"),
            skill_path: skill_path.to_string(),
            skill_folder_hash: hash.to_string(),
            installed_at: now.clone(),
            updated_at: now,
        }
    }
}

pub fn compute_folder_hash(dir: &Path) -> Result<String> {
    use sha1::{Digest, Sha1};
    use walkdir::WalkDir;

    let mut hasher = Sha1::new();
    let mut paths: Vec<_> = WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.into_path())
        .collect();
    paths.sort();

    for path in paths {
        let contents = std::fs::read(&path).map_err(|e| {
            AppError::Io(std::io::Error::new(
                e.kind(),
                format!("{}: {e}", path.display()),
            ))
        })?;
        hasher.update(&contents);
    }

    Ok(format!("{:x}", hasher.finalize()))
}
