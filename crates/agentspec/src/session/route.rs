//! Route session context into fleet panes.
//!
//! Brief routing is intentionally conservative: it excludes system/developer
//! messages, tool calls, and tool results. Full routing is explicit and uses the
//! existing markdown export renderer.

use crate::error::{AppError, Result};
use crate::ops::fleet;
use crate::session::{self, ir, render};

pub const BRIEF_POLICY_SUMMARY: &str = "brief. Includes metadata and user/assistant text only. Excludes system/developer prompts, tool calls, and tool results.";
pub const FULL_POLICY_SUMMARY: &str = "full. This is an explicit full markdown export and may include tool calls/results captured by the source adapter.";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContextMode {
    Brief,
    Full,
}

#[derive(serde::Serialize)]
pub struct ContextPolicy {
    pub name: &'static str,
    pub default: bool,
    pub requires_explicit_selection: bool,
    pub includes: &'static [&'static str],
    pub excludes: &'static [&'static str],
    pub limits: &'static [&'static str],
}

#[derive(serde::Serialize)]
pub struct RoutingPolicy {
    pub default_context: &'static str,
    pub modes: &'static [ContextPolicy],
    pub safeguards: &'static [&'static str],
}

pub const ROUTING_POLICY: RoutingPolicy = RoutingPolicy {
    default_context: "brief",
    modes: &[
        ContextPolicy {
            name: "brief",
            default: true,
            requires_explicit_selection: false,
            includes: &[
                "source metadata",
                "operator note",
                "recent user text",
                "recent assistant text",
            ],
            excludes: &[
                "system prompts",
                "developer prompts",
                "tool calls",
                "tool results",
                "non-text message blocks",
            ],
            limits: &[
                "last 12 user/assistant messages",
                "2000 characters per rendered message",
            ],
        },
        ContextPolicy {
            name: "full",
            default: false,
            requires_explicit_selection: true,
            includes: &[
                "operator note",
                "rendered markdown export for the source session",
            ],
            excludes: &[],
            limits: &["requires --context full"],
        },
    ],
    safeguards: &[
        "session sync stages markdown handoffs under ~/.agents/sessions/<target>/ instead of writing native tool session stores",
        "session route supports --dry-run to print the exact payload before delivery",
        "route-active and route-fleet only auto-route sessions matched to fleet panes by tool and related working directory",
    ],
};

pub fn render_policy_markdown() -> String {
    let policy = &ROUTING_POLICY;
    let mut out = String::new();
    out.push_str("# Session Routing Policy\n\n");
    out.push_str(&format!(
        "- Default context: {}\n\n",
        policy.default_context
    ));

    for mode in policy.modes {
        let marker = if mode.default { " (default)" } else { "" };
        out.push_str(&format!("## {}{}\n\n", mode.name, marker));
        out.push_str(&format!(
            "- Requires explicit selection: {}\n",
            if mode.requires_explicit_selection {
                "yes"
            } else {
                "no"
            }
        ));
        out.push_str(&format!("- Includes: {}\n", mode.includes.join(", ")));
        if mode.excludes.is_empty() {
            out.push_str("- Excludes: none\n");
        } else {
            out.push_str(&format!("- Excludes: {}\n", mode.excludes.join(", ")));
        }
        out.push_str(&format!("- Limits: {}\n\n", mode.limits.join(", ")));
    }

    out.push_str("## Safeguards\n\n");
    for safeguard in policy.safeguards {
        out.push_str(&format!("- {safeguard}\n"));
    }
    out
}

pub struct RouteReport {
    pub source: String,
    pub session_id: String,
    pub pane: String,
    pub context: ContextMode,
    pub bytes: usize,
}

pub struct RoutePreview {
    pub source: String,
    pub session_id: String,
    pub pane: String,
    pub context: ContextMode,
    pub bytes: usize,
    pub markdown: String,
}

pub fn route_session(
    source: &str,
    pane: &str,
    id: Option<&str>,
    last: bool,
    backend: fleet::BackendSelection,
    context: ContextMode,
    note: Option<&str>,
) -> Result<RouteReport> {
    let preview = preview_route_context(source, pane, id, last, context, note)?;
    fleet::send_text(backend, pane, &preview.markdown)?;

    Ok(RouteReport {
        source: preview.source,
        session_id: preview.session_id,
        pane: preview.pane,
        context: preview.context,
        bytes: preview.bytes,
    })
}

pub fn preview_route_context(
    source: &str,
    pane: &str,
    id: Option<&str>,
    last: bool,
    context: ContextMode,
    note: Option<&str>,
) -> Result<RoutePreview> {
    let adapter = session::get_adapter(source)?;
    let sess = if last {
        adapter.latest_session()?
    } else if let Some(id) = id {
        adapter.load_session(id)?
    } else {
        return Err(AppError::Other("provide a session ID or use --last".into()));
    };

    let message = render_context(&sess, context, note);
    let bytes = message.len();

    Ok(RoutePreview {
        source: source.to_string(),
        session_id: sess.id,
        pane: pane.to_string(),
        context,
        bytes,
        markdown: message,
    })
}

pub fn render_context(session: &ir::SessionIR, context: ContextMode, note: Option<&str>) -> String {
    match context {
        ContextMode::Brief => render_brief_context(session, note),
        ContextMode::Full => render_full_context(session, note),
    }
}

pub fn render_brief_context(session: &ir::SessionIR, note: Option<&str>) -> String {
    let mut out = String::new();
    out.push_str("# Routed Session Context\n\n");
    out.push_str("Context policy: ");
    out.push_str(BRIEF_POLICY_SUMMARY);
    out.push_str("\n\n");
    render_metadata(&mut out, session);
    if let Some(note) = note.filter(|n| !n.trim().is_empty()) {
        out.push_str("\n## Operator Note\n\n");
        out.push_str(note.trim());
        out.push('\n');
    }

    out.push_str("\n## Recent Conversation\n\n");
    let mut rendered = 0usize;
    for msg in session
        .messages
        .iter()
        .filter(|msg| matches!(msg.role, ir::RoleIR::User | ir::RoleIR::Assistant))
        .rev()
        .take(12)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
    {
        let text = msg
            .content
            .iter()
            .filter_map(|block| match block {
                ir::ContentBlockIR::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        if text.trim().is_empty() {
            continue;
        }
        rendered += 1;
        out.push_str(&format!("### {:?}\n\n", msg.role));
        out.push_str(&truncate_chars(text.trim(), 2000));
        out.push_str("\n\n");
    }
    if rendered == 0 {
        out.push_str("(no routable user/assistant text)\n");
    }
    out
}

fn render_full_context(session: &ir::SessionIR, note: Option<&str>) -> String {
    let mut out = String::new();
    out.push_str("# Routed Session Context\n\n");
    out.push_str("Context policy: ");
    out.push_str(FULL_POLICY_SUMMARY);
    out.push_str("\n\n");
    if let Some(note) = note.filter(|n| !n.trim().is_empty()) {
        out.push_str("## Operator Note\n\n");
        out.push_str(note.trim());
        out.push_str("\n\n");
    }
    out.push_str(&render::render_markdown(session));
    out
}

fn render_metadata(out: &mut String, session: &ir::SessionIR) {
    out.push_str("## Source\n\n");
    out.push_str(&format!("- Tool: {}\n", session.tool_slug));
    out.push_str(&format!("- Session: {}\n", session.id));
    if let Some(cwd) = &session.cwd {
        out.push_str(&format!("- Cwd: {cwd}\n"));
    }
    if let Some(project) = &session.project {
        out.push_str(&format!("- Project: {project}\n"));
    }
    if let Some(branch) = &session.branch {
        out.push_str(&format!("- Branch: {branch}\n"));
    }
    if let Some(started_at) = session.started_at {
        out.push_str(&format!("- Started: {started_at}\n"));
    }
    if let Some(summary) = &session.summary {
        out.push_str(&format!("- Summary: {}\n", first_line(summary)));
    }
    if let Some(prompt) = &session.first_prompt {
        out.push_str(&format!("- First prompt: {}\n", first_line(prompt)));
    }
}

fn first_line(s: &str) -> &str {
    s.lines().next().unwrap_or(s)
}

fn truncate_chars(s: &str, max: usize) -> String {
    let mut out = String::new();
    for (idx, c) in s.chars().enumerate() {
        if idx >= max {
            out.push_str("\n\n[truncated]");
            return out;
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn brief_context_excludes_tools_and_system_messages() {
        let session = ir::SessionIR {
            id: "s1".into(),
            tool_slug: "codex".into(),
            cwd: Some("/repo".into()),
            started_at: Some(Utc::now()),
            ended_at: None,
            first_prompt: Some("fix tests".into()),
            summary: None,
            project: None,
            branch: None,
            messages: vec![
                ir::MessageIR {
                    role: ir::RoleIR::System,
                    content: vec![ir::ContentBlockIR::Text {
                        text: "secret system".into(),
                    }],
                    timestamp: None,
                },
                ir::MessageIR {
                    role: ir::RoleIR::User,
                    content: vec![ir::ContentBlockIR::Text {
                        text: "please inspect".into(),
                    }],
                    timestamp: None,
                },
                ir::MessageIR {
                    role: ir::RoleIR::Assistant,
                    content: vec![
                        ir::ContentBlockIR::Text {
                            text: "I will check".into(),
                        },
                        ir::ContentBlockIR::ToolUse {
                            name: "shell".into(),
                            input: "cat secrets".into(),
                            id: None,
                        },
                    ],
                    timestamp: None,
                },
            ],
            files_touched: vec![],
            tools_used: vec![],
            model: None,
            extensions: Default::default(),
        };

        let rendered = render_brief_context(&session, Some("continue carefully"));
        assert!(rendered.contains("please inspect"));
        assert!(rendered.contains("I will check"));
        assert!(rendered.contains("continue carefully"));
        assert!(!rendered.contains("secret system"));
        assert!(!rendered.contains("cat secrets"));
    }
}
