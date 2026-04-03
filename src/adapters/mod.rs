pub mod agentskills;
pub mod claude;
pub mod gemini;

use std::path::Path;

use crate::error::Result;
use crate::ir::Resource;

/// Trait for vendor-specific format adapters.
/// Each adapter can parse a vendor's file format into the canonical IR
/// and emit the IR back into that vendor's format.
#[allow(dead_code)]
pub trait Adapter: Send + Sync {
    /// Vendor identifier (e.g. "agentskills", "claude-code", "gemini-cli")
    fn vendor(&self) -> &str;

    /// Parse a file into the canonical IR.
    fn parse(&self, path: &Path) -> Result<Resource>;

    /// Emit the canonical IR back to the vendor's file format.
    fn emit(&self, resource: &Resource) -> Result<String>;

    /// Validate vendor-specific constraints beyond the base IR.
    fn validate(&self, resource: &Resource) -> Vec<String>;
}

/// Get all registered adapters.
#[allow(dead_code)]
pub fn all_adapters() -> Vec<Box<dyn Adapter>> {
    vec![
        Box::new(agentskills::AgentSkillsAdapter),
        Box::new(claude::ClaudeAdapter),
        Box::new(gemini::GeminiAdapter),
    ]
}

/// Find the best adapter for a given file path.
pub fn adapter_for_path(path: &Path) -> Option<Box<dyn Adapter>> {
    let filename = path.file_name()?.to_str()?;

    if filename == "SKILL.md" {
        return Some(Box::new(agentskills::AgentSkillsAdapter));
    }

    // Check parent directory to determine context
    let parent = path.parent()?.file_name()?.to_str()?;
    let grandparent = path.parent()?.parent()?.file_name()?.to_str()?;

    if parent == "agents" || grandparent == ".claude" {
        return Some(Box::new(claude::ClaudeAdapter));
    }
    if grandparent == ".gemini" {
        return Some(Box::new(gemini::GeminiAdapter));
    }

    // Default: try Claude format for .md files in agents dirs
    if filename.ends_with(".md") {
        return Some(Box::new(claude::ClaudeAdapter));
    }

    None
}
