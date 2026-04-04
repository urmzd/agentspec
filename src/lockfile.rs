use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::error::Result;

/// Legacy `.skill-lock.json` v3 format.
/// Retained only for migration to the new inventory config.
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

    pub fn empty() -> Self {
        Self {
            version: 3,
            skills: HashMap::new(),
            dismissed: HashMap::new(),
            last_selected_agents: Vec::new(),
        }
    }
}
