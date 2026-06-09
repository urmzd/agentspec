pub mod agents_md;
pub mod agentskills;
pub mod claude;
pub mod claude_md;
pub mod gemini;
pub mod instruction_file;
pub mod llms_txt;

#[cfg(test)]
mod conformance_tests;

use std::path::Path;

use crate::error::Result;
use crate::ir::Resource;

/// Trait for vendor-specific format adapters.
/// Each adapter can parse a vendor's file format into the canonical IR
/// and emit the IR back into that vendor's format.
pub trait Adapter: Send + Sync {
    /// Vendor identifier (e.g. "agentskills", "claude-code", "gemini-cli")
    fn vendor(&self) -> &str;

    /// Parse a file into the canonical IR.
    fn parse(&self, path: &Path) -> Result<Resource>;

    /// Emit the IR back into this vendor's file format.
    /// Invariant: `parse(emit(parse(f)))` yields the same `Resource` as `parse(f)`.
    fn emit(&self, resource: &Resource) -> Result<String>;

    /// Validate vendor-specific constraints beyond the base IR.
    fn validate(&self, resource: &Resource) -> Vec<String>;
}

/// Sort metadata keys so emitted frontmatter is deterministic
/// (`Resource::metadata` is a `HashMap`).
pub(crate) fn sorted_yaml_entries(
    map: &std::collections::HashMap<String, serde_yaml_ng::Value>,
) -> Vec<(String, serde_yaml_ng::Value)> {
    let mut entries: Vec<_> = map.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    entries
}

/// Find the best adapter for a given file path.
pub fn adapter_for_path(path: &Path) -> Option<Box<dyn Adapter>> {
    let filename = path.file_name()?.to_str()?;

    // Route by well-known filename first
    match filename {
        "SKILL.md" => return Some(Box::new(agentskills::AgentSkillsAdapter)),
        "AGENTS.md" => return Some(Box::new(agents_md::AgentsMdAdapter)),
        "CLAUDE.md" => return Some(Box::new(claude_md::ClaudeMdAdapter)),
        "llms.txt" => return Some(Box::new(llms_txt::LlmsTxtAdapter)),
        // Instruction files — editor-specific
        "GEMINI.md"
        | ".cursorrules"
        | ".clinerules"
        | ".windsurfrules"
        | "copilot-instructions.md"
        | "codex-instructions.md" => {
            return Some(Box::new(instruction_file::InstructionFileAdapter));
        }
        _ => {}
    }

    // Check parent directory to determine context. Shallow paths may lack
    // one or both ancestors; the .md fallback below must still apply.
    let parent = path
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str());
    let grandparent = path
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str());

    // Gemini first: its agents also live in an `agents/` directory
    // (~/.gemini/agents/), which the Claude check below would claim.
    if grandparent == Some(".gemini") {
        return Some(Box::new(gemini::GeminiAdapter));
    }
    if parent == Some("agents") || grandparent == Some(".claude") {
        return Some(Box::new(claude::ClaudeAdapter));
    }

    // Instruction file directories (e.g. .cursor/rules/)
    if parent == Some(".cursor") && filename == "rules" {
        return Some(Box::new(instruction_file::InstructionFileAdapter));
    }

    // Default: try Claude format for .md files in agents dirs
    if filename.ends_with(".md") {
        return Some(Box::new(claude::ClaudeAdapter));
    }

    None
}
