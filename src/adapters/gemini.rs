use std::path::Path;

use crate::error::Result;
use crate::frontmatter;
use crate::ir::Resource;

use super::Adapter;

/// Adapter for Gemini CLI agent definition format.
/// Frontmatter: name, description, tools, model, temperature, max_turns, timeout_mins, kind
pub struct GeminiAdapter;

#[derive(serde::Deserialize)]
struct RawGeminiFm {
    name: String,
    description: String,
    #[serde(default)]
    tools: Option<Vec<String>>,
    model: Option<String>,
    temperature: Option<f64>,
    max_turns: Option<u32>,
    timeout_mins: Option<u32>,
    kind: Option<String>,
    #[serde(default, rename = "mcpServers")]
    mcp_servers: Option<serde_yaml::Value>,
}

impl Adapter for GeminiAdapter {
    fn vendor(&self) -> &str {
        "gemini-cli"
    }

    fn parse(&self, path: &Path) -> Result<Resource> {
        let content = std::fs::read_to_string(path)?;
        let parsed = frontmatter::parse(&content)?;
        let fm: RawGeminiFm = serde_yaml::from_str(&parsed.frontmatter)?;

        let mut r = Resource::new_agent(fm.name, fm.description, parsed.body);
        r.tools = fm.tools;
        r.model = fm.model;
        r.temperature = fm.temperature;
        r.max_turns = fm.max_turns;
        r.timeout_mins = fm.timeout_mins;
        // Stash Gemini-specific fields
        let mut ext = serde_yaml::Mapping::new();
        if let Some(kind) = fm.kind {
            ext.insert(
                serde_yaml::Value::String("kind".into()),
                serde_yaml::Value::String(kind),
            );
        }
        if let Some(mcp) = fm.mcp_servers {
            ext.insert(serde_yaml::Value::String("mcpServers".into()), mcp);
        }
        if !ext.is_empty() {
            r.extensions
                .insert("gemini-cli".into(), serde_yaml::Value::Mapping(ext));
        }

        Ok(r)
    }

    fn emit(&self, resource: &Resource) -> Result<String> {
        let mut fm = String::from("---\n");
        fm.push_str(&format!("name: {}\n", resource.name));
        fm.push_str("description: |\n");
        for line in resource.description.lines() {
            fm.push_str(&format!("  {line}\n"));
        }
        if let Some(tools) = &resource.tools {
            fm.push_str("tools:\n");
            for t in tools {
                fm.push_str(&format!("  - {t}\n"));
            }
        }
        if let Some(model) = &resource.model {
            fm.push_str(&format!("model: {model}\n"));
        }
        if let Some(temp) = resource.temperature {
            fm.push_str(&format!("temperature: {temp}\n"));
        }
        if let Some(mt) = resource.max_turns {
            fm.push_str(&format!("max_turns: {mt}\n"));
        }
        if let Some(tm) = resource.timeout_mins {
            fm.push_str(&format!("timeout_mins: {tm}\n"));
        }

        // Gemini-specific extensions
        if let Some(serde_yaml::Value::Mapping(ext)) = resource.extensions.get("gemini-cli") {
            if let Some(kind) = ext.get(serde_yaml::Value::String("kind".into()))
                && let Some(k) = kind.as_str()
            {
                fm.push_str(&format!("kind: {k}\n"));
            }
            if let Some(mcp) = ext.get(serde_yaml::Value::String("mcpServers".into())) {
                let val = serde_yaml::to_string(mcp).unwrap_or_default();
                fm.push_str(&format!("mcpServers: {}", val.trim_start()));
            }
        }

        fm.push_str("---\n\n");
        fm.push_str(&resource.body);
        Ok(fm)
    }

    fn validate(&self, resource: &Resource) -> Vec<String> {
        let mut issues = Vec::new();
        if resource.name.is_empty() {
            issues.push("name is required".into());
        }
        if resource.description.is_empty() {
            issues.push("description is required".into());
        }
        if let Some(temp) = resource.temperature
            && !(0.0..=2.0).contains(&temp)
        {
            issues.push("temperature must be between 0.0 and 2.0".into());
        }
        if let Some(model) = &resource.model
            && !model.starts_with("gemini-")
        {
            issues.push(format!("model '{model}' may not be a valid Gemini model"));
        }
        issues
    }
}
