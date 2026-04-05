use std::path::Path;

use crate::error::Result;
use crate::ir::{Resource, ResourceKind};

use super::Adapter;

/// Adapter for AGENTS.md project configuration files.
/// These are plain markdown (no YAML frontmatter). Name comes from the first
/// `#` heading, description from the first paragraph after it.
pub struct AgentsMdAdapter;

impl Adapter for AgentsMdAdapter {
    fn vendor(&self) -> &str {
        "agents-md"
    }

    fn parse(&self, path: &Path) -> Result<Resource> {
        let content = std::fs::read_to_string(path)?;
        let (name, description, body) = parse_heading_doc(&content);

        let r = Resource::new_simple(ResourceKind::ProjectConfig, name, description, body);
        Ok(r)
    }

    fn validate(&self, resource: &Resource) -> Vec<String> {
        let mut issues = Vec::new();
        if resource.name.is_empty() {
            issues.push("AGENTS.md should have a top-level # heading".into());
        }
        issues
    }
}

/// Extract name from first `# heading`, description from first paragraph, rest as body.
/// Public for reuse by `claude_md` adapter.
pub fn parse_heading_doc(content: &str) -> (String, String, String) {
    let mut lines = content.lines();
    let mut name = String::new();
    let mut desc_lines = Vec::new();
    let mut body_start = 0;
    let mut in_desc = false;
    let mut offset = 0;

    for line in &mut lines {
        let line_len = line.len() + 1; // +1 for newline
        if name.is_empty() {
            if let Some(heading) = line.strip_prefix("# ") {
                name = heading.trim().to_string();
                in_desc = true;
                offset += line_len;
                continue;
            }
        } else if in_desc {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                if !desc_lines.is_empty() {
                    // End of first paragraph
                    body_start = offset + line_len;
                    break;
                }
            } else if trimmed.starts_with('#') {
                // Next heading — no description paragraph
                body_start = offset;
                break;
            } else {
                desc_lines.push(trimmed.to_string());
            }
        }
        offset += line_len;
    }

    let description = desc_lines.join(" ");
    let body = if body_start > 0 && body_start < content.len() {
        content[body_start..].trim().to_string()
    } else if name.is_empty() {
        content.to_string()
    } else {
        // Everything after the heading line
        let after_heading = content
            .find(&format!("# {name}"))
            .map(|i| {
                let rest = &content[i..];
                rest.find('\n')
                    .map(|nl| &content[i + nl + 1..])
                    .unwrap_or("")
            })
            .unwrap_or("");
        after_heading.trim().to_string()
    };

    (name, description, body)
}
