use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::error::Result;
use crate::frontmatter;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillFrontmatter {
    pub name: String,
    pub description: String,
    #[serde(rename = "allowed-tools")]
    pub allowed_tools: Option<String>,
    pub license: Option<String>,
    pub compatibility: Option<String>,
    pub metadata: Option<HashMap<String, serde_yaml::Value>>,
    #[serde(rename = "user-invocable")]
    pub user_invocable: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct Skill {
    pub frontmatter: SkillFrontmatter,
    pub body: String,
    pub path: std::path::PathBuf,
}

impl Skill {
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let parsed = frontmatter::parse(&content)?;
        let fm: SkillFrontmatter = serde_yaml::from_str(&parsed.frontmatter)?;
        Ok(Self {
            frontmatter: fm,
            body: parsed.body,
            path: path.to_path_buf(),
        })
    }

    pub fn validate(&self) -> Vec<String> {
        let mut issues = Vec::new();
        let fm = &self.frontmatter;

        if fm.name.is_empty() {
            issues.push("name is required".into());
        }
        if fm.name.len() > 64 {
            issues.push("name must be <= 64 characters".into());
        }
        if fm.name != fm.name.to_lowercase() {
            issues.push("name must be lowercase".into());
        }
        if fm.name.starts_with('-') || fm.name.ends_with('-') {
            issues.push("name must not start or end with a hyphen".into());
        }
        if fm.name.contains("--") {
            issues.push("name must not contain consecutive hyphens".into());
        }
        if !fm
            .name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            issues.push("name must contain only lowercase letters, digits, and hyphens".into());
        }
        if fm.description.is_empty() {
            issues.push("description is required".into());
        }
        if fm.description.len() > 1024 {
            issues.push("description must be <= 1024 characters".into());
        }
        if let Some(compat) = &fm.compatibility
            && compat.len() > 500
        {
            issues.push("compatibility must be <= 500 characters".into());
        }

        issues
    }
}
