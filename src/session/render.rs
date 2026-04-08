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

    if !session.tools_used.is_empty() {
        out.push_str(&format!(
            "- **Tools Used**: {}\n",
            session.tools_used.join(", ")
        ));
    }

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
                    let suffix = if content.len() > TOOL_RESULT_MAX_LEN {
                        " ..."
                    } else {
                        ""
                    };
                    out.push_str(&format!(
                        "\n> **{label}**{}\n> ```\n> {}\n> ```\n",
                        if content.len() > TOOL_RESULT_MAX_LEN {
                            " (truncated)"
                        } else {
                            ""
                        },
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
