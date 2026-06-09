use std::path::Path;

use crate::error::Result;
use crate::ir::{Resource, ResourceKind};

use super::Adapter;

/// Generic adapter for editor-specific instruction files:
/// .cursorrules, .clinerules, .windsurfrules, copilot-instructions.md,
/// codex-instructions.md, GEMINI.md, etc.
///
/// These are plain markdown or text files — no YAML frontmatter expected.
/// Name comes from the filename, description from the first paragraph.
pub struct InstructionFileAdapter;

impl Adapter for InstructionFileAdapter {
    fn vendor(&self) -> &str {
        "instruction-file"
    }

    fn parse(&self, path: &Path) -> Result<Resource> {
        let content = std::fs::read_to_string(path)?;
        let filename = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        let (name, description, body) = parse_instruction_file(&content, &filename);

        let r = Resource::new_simple(ResourceKind::InstructionFile, name, description, body);
        Ok(r)
    }

    fn emit(&self, resource: &Resource) -> Result<String> {
        // A heading is only re-emitted alongside a description: when the
        // description is empty the parsed name may have come from the
        // filename, and a fabricated heading would change the parse.
        if resource.description.is_empty() {
            return Ok(resource.body.clone());
        }
        Ok(format!(
            "# {}\n\n{}\n\n{}\n",
            resource.name, resource.description, resource.body
        ))
    }

    fn validate(&self, resource: &Resource) -> Vec<String> {
        let mut issues = Vec::new();
        if resource.body.is_empty() {
            issues.push("Instruction file has no content".into());
        }
        issues
    }
}

/// Parse an instruction file: try `# heading` first, fall back to filename.
fn parse_instruction_file(content: &str, filename: &str) -> (String, String, String) {
    let mut name = String::new();
    let mut desc_lines = Vec::new();
    let mut body_start = 0;
    let mut in_desc = false;
    let mut offset = 0;

    for line in content.lines() {
        let line_len = line.len() + 1;
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
                    body_start = offset + line_len;
                    break;
                }
            } else if trimmed.starts_with('#') {
                body_start = offset;
                break;
            } else {
                desc_lines.push(trimmed.to_string());
            }
        }
        offset += line_len;
    }

    // Fall back to filename as name
    if name.is_empty() {
        name = filename.to_string();
    }

    let description = desc_lines.join(" ");
    let body = if body_start > 0 && body_start < content.len() {
        content[body_start..].trim().to_string()
    } else if description.is_empty() {
        // Entire content is the body
        content.trim().to_string()
    } else {
        // Everything after heading + description paragraph
        let after = content
            .find(&format!("# {}", name.trim_start_matches("# ")))
            .map(|i| {
                let rest = &content[i..];
                rest.find('\n')
                    .map(|nl| &content[i + nl + 1..])
                    .unwrap_or("")
            })
            .unwrap_or("");
        after.trim().to_string()
    };

    (name, description, body)
}
