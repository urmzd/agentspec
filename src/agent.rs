use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::error::Result;
use crate::frontmatter;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentFrontmatter {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub tools: Option<StringOrVec>,
    pub disallowed_tools: Option<Vec<String>>,
    pub model: Option<String>,
    pub max_turns: Option<u32>,
    pub color: Option<String>,
    pub permission_mode: Option<String>,
    pub skills: Option<Vec<String>>,
    pub background: Option<bool>,
    pub effort: Option<String>,
    pub isolation: Option<String>,
    pub initial_prompt: Option<String>,
    pub temperature: Option<f64>,
    pub timeout_mins: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StringOrVec {
    String(String),
    Vec(Vec<String>),
}

impl StringOrVec {
    pub fn as_list(&self) -> Vec<String> {
        match self {
            Self::String(s) => s
                .split(',')
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
                .collect(),
            Self::Vec(v) => v.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AgentDef {
    pub frontmatter: AgentFrontmatter,
    pub body: String,
    pub path: std::path::PathBuf,
}

impl AgentDef {
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let parsed = frontmatter::parse(&content)?;
        let fm: AgentFrontmatter = serde_yaml::from_str(&parsed.frontmatter)?;
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
        if fm.description.is_empty() {
            issues.push("description is required".into());
        }
        if let Some(color) = &fm.color {
            let valid = [
                "red", "blue", "green", "yellow", "purple", "orange", "pink", "cyan",
            ];
            if !valid.contains(&color.as_str()) {
                issues.push(format!(
                    "invalid color '{color}', must be one of: {}",
                    valid.join(", ")
                ));
            }
        }
        if let Some(model) = &fm.model {
            let valid = ["sonnet", "opus", "haiku", "inherit"];
            if !valid.contains(&model.as_str())
                && !model.starts_with("claude-")
                && !model.starts_with("gemini-")
            {
                issues.push(format!("unrecognized model '{model}'"));
            }
        }
        if let Some(temp) = fm.temperature
            && !(0.0..=2.0).contains(&temp)
        {
            issues.push("temperature must be between 0.0 and 2.0".into());
        }

        issues
    }
}
