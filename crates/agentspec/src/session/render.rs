use super::adapters;
use super::ir::{ContentBlockIR, RoleIR, SessionIR};

const TOOL_RESULT_MAX_LEN: usize = 500;

pub fn render_markdown(session: &SessionIR) -> String {
    let mut out = String::new();

    let source_label = adapters::tool_name_for_slug(&session.tool_slug);

    out.push_str("# Session Handoff\n\n");
    out.push_str("## Context\n");
    out.push_str(&format!("- **Source**: {source_label}\n"));
    if let Some(ref cwd) = session.cwd {
        out.push_str(&format!("- **Project**: {cwd}\n"));
    }
    if let Some(ref ts) = session.started_at {
        out.push_str(&format!(
            "- **Date**: {}\n",
            ts.format("%Y-%m-%d %H:%M UTC")
        ));
    }

    if let Some(ref branch) = session.branch {
        out.push_str(&format!("- **Branch**: {branch}\n"));
    }
    if let Some(ref summary) = session.summary {
        out.push_str(&format!("- **Summary**: {summary}\n"));
    }

    if !session.tools_used.is_empty() {
        out.push_str(&format!(
            "- **Tools Used**: {}\n",
            session.tools_used.join(", ")
        ));
    }

    if !session.files_touched.is_empty() {
        out.push_str("\n## Files Touched\n");
        for f in &session.files_touched {
            out.push_str(&format!("- `{f}`\n"));
        }
    }

    render_checkpoints(&mut out, session);
    render_refs(&mut out, session);

    out.push_str("\n## Conversation\n");

    for msg in &session.messages {
        let role_label = match msg.role {
            RoleIR::User => "User",
            RoleIR::Assistant => "Assistant",
            RoleIR::System => "System",
            RoleIR::Tool => "Tool",
        };

        if let Some(ts) = msg.timestamp {
            out.push_str(&format!("\n### {role_label} ({})\n", ts.format("%H:%M:%S")));
        } else {
            out.push_str(&format!("\n### {role_label}\n"));
        }

        for block in &msg.content {
            match block {
                ContentBlockIR::Text { text } => {
                    out.push('\n');
                    out.push_str(text);
                    out.push('\n');
                }
                ContentBlockIR::ToolUse { name, input, .. } => {
                    out.push_str(&format!("\n> **Tool: {name}**\n"));
                    let compact: String = input.chars().take(200).collect();
                    if !compact.is_empty() {
                        out.push_str(&format!(
                            "> ```\n> {}\n> ```\n",
                            compact.replace('\n', "\n> ")
                        ));
                    }
                }
                ContentBlockIR::ToolResult {
                    content, is_error, ..
                } => {
                    let label = if *is_error { "Error" } else { "Result" };
                    let truncated: String = content.chars().take(TOOL_RESULT_MAX_LEN).collect();
                    let is_truncated = content.chars().count() > TOOL_RESULT_MAX_LEN;
                    let suffix = if is_truncated { " ..." } else { "" };
                    out.push_str(&format!(
                        "\n> **{label}**{}\n> ```\n> {}\n> ```\n",
                        if is_truncated { " (truncated)" } else { "" },
                        format!("{truncated}{suffix}").replace('\n', "\n> ")
                    ));
                }
                ContentBlockIR::Unknown { raw } => {
                    let json = serde_json::to_string_pretty(raw).unwrap_or_default();
                    let compact: String = json.chars().take(200).collect();
                    out.push_str(&format!(
                        "\n> **Unknown block**\n> ```json\n> {}\n> ```\n",
                        compact.replace('\n', "\n> ")
                    ));
                }
            }
        }
    }

    out
}

/// Render checkpoint summaries from `extensions["checkpoints"]` if present.
fn render_checkpoints(out: &mut String, session: &SessionIR) {
    let Some(checkpoints) = session
        .extensions
        .get("checkpoints")
        .and_then(|v| v.as_array())
    else {
        return;
    };
    if checkpoints.is_empty() {
        return;
    }
    out.push_str("\n## Checkpoints\n");
    for cp in checkpoints {
        let num = cp.get("number").and_then(|v| v.as_i64()).unwrap_or(0);
        let title = cp
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("(untitled)");
        out.push_str(&format!("\n### Checkpoint {num}: {title}\n"));
        for (key, label) in [
            ("overview", "Overview"),
            ("work_done", "Work done"),
            ("technical_details", "Technical details"),
            ("next_steps", "Next steps"),
        ] {
            if let Some(text) = cp.get(key).and_then(|v| v.as_str())
                && !text.is_empty()
            {
                out.push_str(&format!("- **{label}**: {text}\n"));
            }
        }
    }
}

/// Render cross-references from `extensions["refs"]` if present.
fn render_refs(out: &mut String, session: &SessionIR) {
    let Some(refs) = session.extensions.get("refs").and_then(|v| v.as_array()) else {
        return;
    };
    if refs.is_empty() {
        return;
    }
    out.push_str("\n## References\n");
    for r in refs {
        let ty = r.get("type").and_then(|v| v.as_str()).unwrap_or("ref");
        let val = r.get("value").and_then(|v| v.as_str()).unwrap_or("");
        out.push_str(&format!("- **{ty}**: {val}\n"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn base_session() -> SessionIR {
        SessionIR {
            id: "s1".into(),
            tool_slug: "github-copilot".into(),
            cwd: Some("/repo".into()),
            started_at: None,
            ended_at: None,
            first_prompt: None,
            summary: Some("Did work".into()),
            project: None,
            branch: Some("main".into()),
            messages: vec![],
            files_touched: vec!["src/a.rs".into(), "src/b.rs".into()],
            tools_used: vec![],
            model: None,
            extensions: HashMap::new(),
        }
    }

    #[test]
    fn renders_enriched_sections() {
        let mut s = base_session();
        s.extensions.insert(
            "checkpoints".into(),
            serde_json::json!([{
                "number": 1, "title": "Start", "overview": "ov",
                "work_done": "wd", "technical_details": "td", "next_steps": "ns"
            }]),
        );
        s.extensions.insert(
            "refs".into(),
            serde_json::json!([{ "type": "pr", "value": "#42" }]),
        );
        let md = render_markdown(&s);
        assert!(md.contains("- **Branch**: main"));
        assert!(md.contains("- **Summary**: Did work"));
        assert!(md.contains("## Files Touched"));
        assert!(md.contains("- `src/a.rs`"));
        assert!(md.contains("## Checkpoints"));
        assert!(md.contains("### Checkpoint 1: Start"));
        assert!(md.contains("- **Next steps**: ns"));
        assert!(md.contains("## References"));
        assert!(md.contains("- **pr**: #42"));
    }

    #[test]
    fn multibyte_tool_result_under_char_limit_is_not_marked_truncated() {
        // 300 chars of 3-byte CJK: > 500 bytes but <= 500 chars.
        let content = "日".repeat(300);
        let mut s = base_session();
        s.messages.push(super::super::ir::MessageIR {
            role: RoleIR::Tool,
            timestamp: None,
            content: vec![ContentBlockIR::ToolResult {
                tool_use_id: None,
                content: content.clone(),
                is_error: false,
            }],
        });
        let md = render_markdown(&s);
        assert!(!md.contains("(truncated)"));
        assert!(!md.contains(" ..."));
        assert!(md.contains(&content));
    }

    #[test]
    fn long_tool_result_is_truncated_at_char_boundary() {
        let content = "日".repeat(TOOL_RESULT_MAX_LEN + 1);
        let mut s = base_session();
        s.messages.push(super::super::ir::MessageIR {
            role: RoleIR::Tool,
            timestamp: None,
            content: vec![ContentBlockIR::ToolResult {
                tool_use_id: None,
                content,
                is_error: false,
            }],
        });
        let md = render_markdown(&s);
        assert!(md.contains("(truncated)"));
        assert!(md.contains(" ..."));
    }

    #[test]
    fn omits_enriched_sections_when_absent() {
        let mut s = base_session();
        s.files_touched.clear();
        s.summary = None;
        s.branch = None;
        let md = render_markdown(&s);
        assert!(!md.contains("## Files Touched"));
        assert!(!md.contains("## Checkpoints"));
        assert!(!md.contains("## References"));
    }
}
