use super::*;

const TOOL_RESULT_MAX_LEN: usize = 500;

pub fn render_markdown(session: &Session) -> String {
    let mut out = String::new();

    out.push_str("# Session Handoff\n\n");
    out.push_str("## Context\n");
    out.push_str(&format!("- **Source**: {}\n", session.meta.source.label()));
    if let Some(ref cwd) = session.meta.cwd {
        out.push_str(&format!("- **Project**: {cwd}\n"));
    }
    if let Some(ref ts) = session.meta.started_at {
        out.push_str(&format!(
            "- **Date**: {}\n",
            ts.format("%Y-%m-%d %H:%M UTC")
        ));
    }

    let tools = session.tools_used();
    if !tools.is_empty() {
        out.push_str(&format!("- **Tools Used**: {}\n", tools.join(", ")));
    }

    out.push_str("\n## Conversation\n");

    for msg in &session.messages {
        let role_label = match msg.role {
            Role::User => "User",
            Role::Assistant => "Assistant",
            Role::System => "System",
        };

        if let Some(ts) = msg.timestamp {
            out.push_str(&format!("\n### {role_label} ({})\n", ts.format("%H:%M:%S")));
        } else {
            out.push_str(&format!("\n### {role_label}\n"));
        }

        for block in &msg.content {
            match block {
                ContentBlock::Text(text) => {
                    out.push('\n');
                    out.push_str(text);
                    out.push('\n');
                }
                ContentBlock::ToolUse { name, input } => {
                    out.push_str(&format!("\n> **Tool: {name}**\n"));
                    let compact: String = input.chars().take(200).collect();
                    if !compact.is_empty() {
                        out.push_str(&format!(
                            "> ```\n> {}\n> ```\n",
                            compact.replace('\n', "\n> ")
                        ));
                    }
                }
                ContentBlock::ToolResult { content } => {
                    let truncated: String = content.chars().take(TOOL_RESULT_MAX_LEN).collect();
                    let suffix = if content.len() > TOOL_RESULT_MAX_LEN {
                        " ..."
                    } else {
                        ""
                    };
                    out.push_str(&format!(
                        "\n> **Result**{}\n> ```\n> {}\n> ```\n",
                        if content.len() > TOOL_RESULT_MAX_LEN {
                            " (truncated)"
                        } else {
                            ""
                        },
                        format!("{truncated}{suffix}").replace('\n', "\n> ")
                    ));
                }
            }
        }
    }

    out
}
