use std::path::Path;

use crate::error::Result;
use crate::ir::{Resource, ResourceKind};

use super::Adapter;

/// Adapter for llms.txt files.
/// Format: `# heading`, optional `> blockquote` description, `## sections`.
pub struct LlmsTxtAdapter;

impl Adapter for LlmsTxtAdapter {
    fn vendor(&self) -> &str {
        "llms-txt"
    }

    fn parse(&self, path: &Path) -> Result<Resource> {
        let content = std::fs::read_to_string(path)?;
        let (name, description, body) = parse_llms_txt(&content);

        let mut r = Resource::new_simple(ResourceKind::LlmsTxt, name, description, body);
        r.source_path = Some(path.to_path_buf());
        Ok(r)
    }

    fn emit(&self, resource: &Resource) -> Result<String> {
        let mut out = format!("# {}\n\n", resource.name);
        if !resource.description.is_empty() {
            out.push_str(&format!("> {}\n\n", resource.description));
        }
        out.push_str(&resource.body);
        Ok(out)
    }

    fn validate(&self, resource: &Resource) -> Vec<String> {
        let mut issues = Vec::new();
        if resource.name.is_empty() {
            issues.push("llms.txt should have a top-level # heading".into());
        }
        issues
    }
}

/// Parse llms.txt: `# name`, `> description`, rest as body.
fn parse_llms_txt(content: &str) -> (String, String, String) {
    let mut name = String::new();
    let mut desc_lines = Vec::new();
    let mut body_start = 0;
    let mut offset = 0;
    let mut past_heading = false;
    let mut past_blockquote = false;

    for line in content.lines() {
        let line_len = line.len() + 1;

        if name.is_empty() {
            if let Some(heading) = line.strip_prefix("# ") {
                name = heading.trim().to_string();
                past_heading = true;
                offset += line_len;
                continue;
            }
        } else if past_heading && !past_blockquote {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                if !desc_lines.is_empty() {
                    past_blockquote = true;
                    body_start = offset + line_len;
                }
                offset += line_len;
                continue;
            }
            if let Some(quote) = trimmed.strip_prefix("> ") {
                desc_lines.push(quote.to_string());
                offset += line_len;
                continue;
            }
            // Not a blockquote — this is body
            past_blockquote = true;
            body_start = offset;
        }

        if past_blockquote && body_start == 0 {
            body_start = offset;
        }
        offset += line_len;
    }

    let description = desc_lines.join(" ");
    let body = if body_start > 0 && body_start < content.len() {
        content[body_start..].trim().to_string()
    } else if past_heading {
        String::new()
    } else {
        content.to_string()
    };

    (name, description, body)
}
