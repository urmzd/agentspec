//! Cross-tool session sync and import.
//!
//! `session sync <source> <target>` loads a session from the source tool
//! (reusing the read adapter + IR), renders it to a portable markdown handoff,
//! and stages it in the canonical store `~/.agents/sessions/<target>/<id>.md`.
//!
//! `session import <target> <file>` stages an existing markdown handoff for a
//! target tool the same way.
//!
//! Native session stores are append-only and tool-internal, so agentspec stages
//! a portable handoff keyed by the intended target rather than fabricating a
//! tool-native session file. The path is printed so it can be opened/resumed.

use std::path::{Path, PathBuf};

use console::style;

use crate::config;
use crate::error::{AppError, Result};
use crate::session::{self, adapters, route::ContextMode};

pub struct SyncReport {
    pub source: String,
    pub target: String,
    pub session_id: String,
    pub context: ContextMode,
    pub bytes: usize,
    pub path: PathBuf,
}

/// Canonicalize a user-provided tool name into its session adapter slug.
fn canonical_slug(name: &str) -> Result<String> {
    adapters::adapter_for_tool(name)
        .map(|a| a.tool_slug().to_string())
        .ok_or_else(|| {
            AppError::Other(format!(
                "unknown tool '{name}' (supported: claude, codex, copilot, gemini)"
            ))
        })
}

/// Reduce an arbitrary session id to a safe single-path-component file stem,
/// guarding against path traversal (e.g. a crafted `../../x` session id).
fn safe_stem(id: &str) -> String {
    let cleaned: String = id
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let stem = cleaned.trim_matches(['.', '/'].as_ref());
    if stem.is_empty() {
        "session".to_string()
    } else {
        stem.to_string()
    }
}

fn store_for(target_slug: &str, id: &str) -> PathBuf {
    config::shared_sessions_dir()
        .join(target_slug)
        .join(format!("{}.md", safe_stem(id)))
}

fn write_handoff(dest: &Path, content: &str) -> Result<()> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(dest, content)?;
    Ok(())
}

/// Translate a session from `source` into a portable handoff staged for `target`.
pub fn sync_session(
    source: &str,
    target: &str,
    id: Option<&str>,
    last: bool,
    context: ContextMode,
    note: Option<&str>,
) -> Result<SyncReport> {
    let target_slug = canonical_slug(target)?;
    let adapter = session::get_adapter(source)?;

    let sess = if last {
        adapter.latest_session()?
    } else if let Some(id) = id {
        adapter.load_session(id)?
    } else {
        return Err(AppError::Other("provide a session ID or use --last".into()));
    };

    let markdown = session::route::render_context(&sess, context, note);
    let bytes = markdown.len();
    let dest = store_for(&target_slug, &sess.id);
    write_handoff(&dest, &markdown)?;

    eprintln!(
        "  {} synced {} session {} → {} handoff ({context:?})",
        style("✓").green().bold(),
        adapter.tool_name(),
        sess.id,
        target_slug
    );
    eprintln!("  staged at {}", dest.display());
    Ok(SyncReport {
        source: adapter.tool_slug().to_string(),
        target: target_slug,
        session_id: sess.id,
        context,
        bytes,
        path: dest,
    })
}

/// Stage an external markdown handoff in the canonical store for `target`.
pub fn import_session(target: &str, file: &str) -> Result<()> {
    let target_slug = canonical_slug(target)?;
    let src = Path::new(file);
    if !src.is_file() {
        return Err(AppError::Other(format!("file not found: {file}")));
    }
    let content = std::fs::read_to_string(src)?;
    let id = src
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "imported".into());
    let dest = store_for(&target_slug, &id);
    write_handoff(&dest, &content)?;

    eprintln!(
        "  {} imported handoff → {} ({})",
        style("✓").green().bold(),
        target_slug,
        dest.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_slug_resolves_aliases() {
        assert_eq!(canonical_slug("claude").unwrap(), "claude-code");
        assert_eq!(canonical_slug("copilot").unwrap(), "github-copilot");
        assert_eq!(canonical_slug("gemini").unwrap(), "gemini-cli");
        assert_eq!(canonical_slug("codex").unwrap(), "codex");
        assert!(canonical_slug("nope").is_err());
    }

    #[test]
    fn store_path_is_keyed_by_target() {
        let p = store_for("codex", "abc123");
        assert!(p.ends_with("sessions/codex/abc123.md"));
    }

    #[test]
    fn safe_stem_blocks_path_traversal() {
        // A crafted session id must not escape the per-tool store directory.
        assert_eq!(safe_stem("../../cron.d/backdoor"), "_.._cron.d_backdoor");
        assert_eq!(safe_stem("a/b/c"), "a_b_c");
        assert_eq!(safe_stem(".."), "session");
        assert_eq!(safe_stem(""), "session");
        // Normal ids pass through unchanged.
        assert_eq!(safe_stem("57064d3a-7a5d-4553"), "57064d3a-7a5d-4553");
        let p = store_for("codex", "../../etc/passwd");
        assert!(p.starts_with(config::shared_sessions_dir().join("codex")));
    }
}
