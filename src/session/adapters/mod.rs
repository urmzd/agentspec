pub mod claude;
pub mod codex;

use super::adapter::SessionAdapter;

/// All known session adapters.
pub fn all_session_adapters() -> Vec<Box<dyn SessionAdapter>> {
    vec![
        Box::new(claude::ClaudeSessionAdapter),
        Box::new(codex::CodexSessionAdapter),
    ]
}

/// Only adapters whose session storage exists on disk.
pub fn available_session_adapters() -> Vec<Box<dyn SessionAdapter>> {
    all_session_adapters()
        .into_iter()
        .filter(|a| a.is_available())
        .collect()
}

/// Find adapter by tool slug, with aliases.
pub fn adapter_for_tool(slug: &str) -> Option<Box<dyn SessionAdapter>> {
    let canonical = match slug {
        "claude" => "claude-code",
        _ => slug,
    };
    all_session_adapters()
        .into_iter()
        .find(|a| a.tool_slug() == canonical)
}
