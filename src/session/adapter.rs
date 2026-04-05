use std::path::PathBuf;

use crate::error::Result;

use super::ir::{SessionIR, SessionMetaIR};

/// Trait for vendor-specific session format adapters.
/// Each adapter can discover, list, and load sessions from a specific tool's storage.
pub trait SessionAdapter: Send + Sync {
    /// Tool slug (e.g. "claude-code", "codex").
    fn tool_slug(&self) -> &str;

    /// Human-readable tool name (e.g. "Claude Code", "Codex CLI").
    fn tool_name(&self) -> &str;

    /// Root directories where this tool stores session data.
    fn session_roots(&self) -> Vec<PathBuf>;

    /// Whether this tool's session storage is available on disk.
    fn is_available(&self) -> bool {
        self.session_roots().iter().any(|r| r.exists())
    }

    /// List all sessions (lightweight metadata only, no messages parsed).
    fn list_sessions(&self) -> Result<Vec<SessionMetaIR>>;

    /// Load a full session by ID (including messages).
    fn load_session(&self, id: &str) -> Result<SessionIR>;

    /// Load the most recent session. Default: first from list_sessions().
    fn latest_session(&self) -> Result<SessionIR> {
        let sessions = self.list_sessions()?;
        let latest = sessions.first().ok_or_else(|| {
            crate::error::AppError::Other(format!("No {} sessions found", self.tool_name()))
        })?;
        self.load_session(&latest.id)
    }
}
