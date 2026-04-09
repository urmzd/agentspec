use super::adapters;
use super::ir::SessionMetaIR;
use crate::error::Result;

/// Discover sessions from all available adapters, merged and sorted by date (newest first).
pub fn discover_all_sessions() -> Result<Vec<SessionMetaIR>> {
    let mut all = Vec::new();
    for adapter in adapters::available_session_adapters() {
        if let Ok(sessions) = adapter.list_sessions() {
            all.extend(sessions);
        }
    }
    all.sort_by(|a, b| b.started_at.cmp(&a.started_at));
    Ok(all)
}
