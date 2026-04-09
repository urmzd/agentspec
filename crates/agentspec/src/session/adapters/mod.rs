pub mod claude;
pub mod codex;
pub mod copilot;
pub mod gemini;

use super::adapter::SessionAdapter;

/// All known session adapters.
pub fn all_session_adapters() -> Vec<Box<dyn SessionAdapter>> {
    vec![
        Box::new(claude::ClaudeSessionAdapter),
        Box::new(codex::CodexSessionAdapter),
        Box::new(copilot::CopilotSessionAdapter),
        Box::new(gemini::GeminiSessionAdapter),
    ]
}

/// Only adapters whose session storage exists on disk.
pub fn available_session_adapters() -> Vec<Box<dyn SessionAdapter>> {
    all_session_adapters()
        .into_iter()
        .filter(|a| a.is_available())
        .collect()
}

/// Find adapter by user-provided name, with aliases.
pub fn adapter_for_tool(slug: &str) -> Option<Box<dyn SessionAdapter>> {
    let canonical = match slug {
        "claude" | "claude-code" => "claude-code",
        "codex" => "codex",
        "copilot" | "github-copilot" => "github-copilot",
        "gemini" | "gemini-cli" => "gemini-cli",
        other => other,
    };
    all_session_adapters()
        .into_iter()
        .find(|a| a.tool_slug() == canonical)
}

/// Human-readable name for a tool slug (falls back to slug if unknown).
pub fn tool_name_for_slug(slug: &str) -> String {
    adapter_for_tool(slug)
        .map(|a| a.tool_name().to_string())
        .unwrap_or_else(|| slug.to_string())
}
