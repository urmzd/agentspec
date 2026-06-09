//! Permission profile sync.
//!
//! A portable `~/.agents/permissions.yml` profile describes allow/deny rules in
//! a tool-agnostic canonical form. `permissions sync` translates the profile
//! into each tool's native allowlist and writes it into the tool's settings:
//!
//! - Claude Code: `permissions.allow` in `~/.claude/settings.json`
//!   (e.g. `"Bash(cat *)"`, `"Read(**)"`)
//! - Gemini CLI: `tools.allowed` in `~/.gemini/settings.json`
//!   (e.g. `"run_shell_command(cat)"`, `"read_file(**)"`)
//!
//! Pre-existing, user-authored entries are preserved: agentspec tracks the
//! entries it manages via a parallel sentinel key, so a sync only adds/removes
//! its own entries and never clobbers ones the user added by hand.

use std::collections::BTreeSet;

use console::style;
use serde::{Deserialize, Serialize};

use crate::config;
use crate::error::{AppError, Result};
use crate::jsonfile;
use crate::tools;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PermissionProfile {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allow: Vec<PermissionRule>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deny: Vec<PermissionRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRule {
    pub kind: RuleKind,
    /// For shell rules: the executable (e.g. `cat`, `git`). For mcp_tool: server.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cmd: Option<String>,
    /// Argument/path glob (e.g. `*`, `*.rs`, `/tmp/**`). For mcp_tool: tool name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    /// Human note, ignored by translators.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RuleKind {
    Shell,
    FileRead,
    FileWrite,
    Network,
    McpTool,
    Wildcard,
}

/// A per-tool translation target.
struct Translator {
    slug: &'static str,
    allow_key: &'static str,
    sentinel_key: &'static str,
    render: fn(&PermissionRule) -> Option<String>,
}

fn translators() -> Vec<Translator> {
    vec![
        Translator {
            slug: "claude-code",
            allow_key: "permissions.allow",
            sentinel_key: "permissions.__agentspec_managed",
            render: render_claude,
        },
        Translator {
            slug: "gemini-cli",
            allow_key: "tools.allowed",
            sentinel_key: "tools.__agentspec_managed",
            render: render_gemini,
        },
    ]
}

fn render_claude(rule: &PermissionRule) -> Option<String> {
    let cmd = rule.cmd.as_deref();
    let pat = rule.pattern.as_deref();
    let s = match rule.kind {
        RuleKind::Shell => {
            let inner = match (cmd, pat) {
                (Some(c), Some(p)) => format!("{c} {p}"),
                (Some(c), None) => c.to_string(),
                (None, Some(p)) => p.to_string(),
                (None, None) => "*".to_string(),
            };
            format!("Bash({inner})")
        }
        RuleKind::FileRead => format!("Read({})", pat.unwrap_or("*")),
        RuleKind::FileWrite => format!("Write({})", pat.unwrap_or("*")),
        RuleKind::Network => format!("WebFetch({})", pat.unwrap_or("*")),
        RuleKind::McpTool => format!("mcp__{}__{}", cmd.unwrap_or("*"), pat.unwrap_or("*")),
        RuleKind::Wildcard => "*".to_string(),
    };
    Some(s)
}

fn render_gemini(rule: &PermissionRule) -> Option<String> {
    let cmd = rule.cmd.as_deref();
    let pat = rule.pattern.as_deref();
    match rule.kind {
        RuleKind::Shell => Some(format!("run_shell_command({})", cmd.unwrap_or("*"))),
        RuleKind::FileRead => Some(format!("read_file({})", pat.unwrap_or("*"))),
        RuleKind::FileWrite => Some(format!("write_file({})", pat.unwrap_or("*"))),
        // Gemini has no canonical network / mcp-tool / wildcard allowlist syntax.
        RuleKind::Network | RuleKind::McpTool | RuleKind::Wildcard => None,
    }
}

#[derive(Debug, Serialize)]
pub struct SyncResult {
    pub tool: String,
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub kept: Vec<String>,
}

fn load_profile() -> Result<PermissionProfile> {
    let path = config::permissions_file();
    if !path.exists() {
        return Err(AppError::Other(format!(
            "no permission profile at {}. Run `agentspec permissions init` first.",
            path.display()
        )));
    }
    let text = std::fs::read_to_string(&path)?;
    let profile: PermissionProfile = serde_yaml_ng::from_str(&text)
        .map_err(|e| AppError::Other(format!("invalid permissions.yml: {e}")))?;
    Ok(profile)
}

const INIT_TEMPLATE: &str = r#"# agentspec permission profile — portable allow/deny rules synced to each tool.
#
# Run `agentspec permissions sync` to translate these into each tool's native
# allowlist:
#   - Claude Code -> permissions.allow in ~/.claude/settings.json
#   - Gemini CLI  -> tools.allowed   in ~/.gemini/settings.json
#
# Rule kinds: shell, file_read, file_write, network, mcp_tool, wildcard
#   shell:      cmd = executable, pattern = arg glob   -> Bash(cmd pattern) / run_shell_command(cmd)
#   file_read:  pattern = path glob                    -> Read(pattern)     / read_file(pattern)
#   file_write: pattern = path glob                    -> Write(pattern)    / write_file(pattern)
#   network:    pattern = url glob                     -> WebFetch(pattern) (Claude only)
#   mcp_tool:   cmd = server, pattern = tool           -> mcp__server__tool (Claude only)
#   wildcard:                                          -> "*"               (Claude only)
allow:
  - kind: shell
    cmd: cat
    pattern: "*"
  - kind: shell
    cmd: ls
  - kind: file_read
    pattern: "**"
deny: []
"#;

/// Scaffold a starter `~/.agents/permissions.yml`.
pub fn init(force: bool) -> Result<()> {
    let path = config::permissions_file();
    if path.exists() && !force {
        return Err(AppError::AlreadyExists(format!(
            "{} already exists (use --force to overwrite)",
            path.display()
        )));
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, INIT_TEMPLATE)?;
    eprintln!("  wrote {}", path.display());
    Ok(())
}

/// Translate the profile into each tool's native allowlist and write it.
pub fn sync(tool: Option<&str>, dry_run: bool, json: bool) -> Result<()> {
    let profile = load_profile()?;

    if !profile.deny.is_empty() {
        eprintln!(
            "  {} deny rules are not yet supported by any target tool's settings; ignoring {} deny rule(s)",
            style("~").yellow(),
            profile.deny.len()
        );
    }

    let mut results = Vec::new();
    for t in translators() {
        if let Some(filter) = tool
            && filter != t.slug
        {
            continue;
        }
        let Some(handle) = tools::find_tool(t.slug) else {
            continue;
        };
        if !handle.is_installed() {
            continue;
        }
        let Some(settings_path) = handle.settings_path() else {
            continue;
        };

        // Render managed entries (dedup, stable order).
        let mut new_managed: Vec<String> =
            profile.allow.iter().filter_map(|r| (t.render)(r)).collect();
        dedup_preserve(&mut new_managed);

        let mut root = jsonfile::read_json(&settings_path);
        let existing = string_array(&root, t.allow_key);
        let sentinel = string_array(&root, t.sentinel_key);

        let sentinel_set: BTreeSet<&String> = sentinel.iter().collect();

        // Entries the user added (never written by us) are preserved.
        let kept: Vec<String> = existing
            .iter()
            .filter(|e| !sentinel_set.contains(e))
            .cloned()
            .collect();
        let kept_set: BTreeSet<&String> = kept.iter().collect();

        let mut final_allow = kept.clone();
        for m in &new_managed {
            if !final_allow.contains(m) {
                final_allow.push(m.clone());
            }
        }

        // The sentinel records only entries agentspec genuinely owns — never an
        // entry the user also added by hand, so a later profile change can't
        // delete a user-authored rule that happened to match a managed one.
        let owned: Vec<String> = new_managed
            .iter()
            .filter(|m| !kept_set.contains(m))
            .cloned()
            .collect();
        let owned_set: BTreeSet<&String> = owned.iter().collect();

        let added: Vec<String> = owned
            .iter()
            .filter(|m| !sentinel_set.contains(m))
            .cloned()
            .collect();
        let removed: Vec<String> = sentinel
            .iter()
            .filter(|s| !owned_set.contains(s))
            .cloned()
            .collect();

        if !dry_run {
            jsonfile::set_dotted(&mut root, t.allow_key, serde_json::json!(final_allow));
            jsonfile::set_dotted(&mut root, t.sentinel_key, serde_json::json!(owned));
            jsonfile::write_json(&settings_path, &root)?;
        }

        results.push(SyncResult {
            tool: t.slug.to_string(),
            added,
            removed,
            kept,
        });
    }

    report_sync(&results, dry_run, json);
    Ok(())
}

/// Show the canonical profile and per-tool rendered allowlists.
pub fn show(tool: Option<&str>, json: bool) -> Result<()> {
    let profile = load_profile()?;

    if json {
        let rendered: Vec<serde_json::Value> = translators()
            .into_iter()
            .filter(|t| tool.is_none_or(|f| f == t.slug))
            .map(|t| {
                let allow: Vec<String> =
                    profile.allow.iter().filter_map(|r| (t.render)(r)).collect();
                serde_json::json!({ "tool": t.slug, "allow": allow })
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "profile": profile,
                "rendered": rendered,
            }))?
        );
        return Ok(());
    }

    for t in translators() {
        if let Some(filter) = tool
            && filter != t.slug
        {
            continue;
        }
        println!("  {} ({})", style(t.slug).bold(), t.allow_key);
        let mut entries: Vec<String> = profile.allow.iter().filter_map(|r| (t.render)(r)).collect();
        dedup_preserve(&mut entries);
        if entries.is_empty() {
            println!("    (no applicable rules)");
        } else {
            for e in entries {
                println!("    {e}");
            }
        }
    }
    Ok(())
}

fn report_sync(results: &[SyncResult], dry_run: bool, json: bool) {
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(results).unwrap_or_else(|_| "[]".into())
        );
        return;
    }
    if results.is_empty() {
        println!("  no installed target tools (claude-code, gemini-cli) found");
        return;
    }
    let verb = if dry_run { "would sync" } else { "synced" };
    for r in results {
        println!(
            "  {} {verb} {} (+{} added, -{} removed, {} kept)",
            style("✓").green().bold(),
            r.tool,
            r.added.len(),
            r.removed.len(),
            r.kept.len()
        );
    }
}

fn string_array(root: &serde_json::Value, key: &str) -> Vec<String> {
    jsonfile::get_dotted(root, key)
        .and_then(|v| v.as_array().cloned())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

fn dedup_preserve(items: &mut Vec<String>) {
    let mut seen = BTreeSet::new();
    items.retain(|i| seen.insert(i.clone()));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shell(cmd: Option<&str>, pat: Option<&str>) -> PermissionRule {
        PermissionRule {
            kind: RuleKind::Shell,
            cmd: cmd.map(String::from),
            pattern: pat.map(String::from),
            comment: None,
        }
    }

    #[test]
    fn claude_shell_rendering() {
        assert_eq!(
            render_claude(&shell(Some("cat"), Some("*"))).unwrap(),
            "Bash(cat *)"
        );
        assert_eq!(render_claude(&shell(Some("ls"), None)).unwrap(), "Bash(ls)");
        assert_eq!(render_claude(&shell(None, Some("*"))).unwrap(), "Bash(*)");
        assert_eq!(render_claude(&shell(None, None)).unwrap(), "Bash(*)");
    }

    #[test]
    fn gemini_shell_uses_cmd_only() {
        assert_eq!(
            render_gemini(&shell(Some("cat"), Some("*"))).unwrap(),
            "run_shell_command(cat)"
        );
        assert_eq!(
            render_gemini(&shell(None, None)).unwrap(),
            "run_shell_command(*)"
        );
    }

    #[test]
    fn gemini_skips_unsupported_kinds() {
        let net = PermissionRule {
            kind: RuleKind::Network,
            cmd: None,
            pattern: Some("https://*".into()),
            comment: None,
        };
        assert!(render_gemini(&net).is_none());
        assert_eq!(render_claude(&net).unwrap(), "WebFetch(https://*)");
    }

    #[test]
    fn file_and_mcp_rules() {
        let read = PermissionRule {
            kind: RuleKind::FileRead,
            cmd: None,
            pattern: Some("/tmp/**".into()),
            comment: None,
        };
        assert_eq!(render_claude(&read).unwrap(), "Read(/tmp/**)");
        assert_eq!(render_gemini(&read).unwrap(), "read_file(/tmp/**)");

        let mcp = PermissionRule {
            kind: RuleKind::McpTool,
            cmd: Some("sr".into()),
            pattern: Some("serve".into()),
            comment: None,
        };
        assert_eq!(render_claude(&mcp).unwrap(), "mcp__sr__serve");
        assert!(render_gemini(&mcp).is_none());
    }

    #[test]
    fn dedup_keeps_first_occurrence_order() {
        let mut v = vec!["a".into(), "b".into(), "a".into(), "c".into()];
        dedup_preserve(&mut v);
        assert_eq!(v, vec!["a", "b", "c"]);
    }

    #[test]
    fn template_parses_as_profile() {
        let p: PermissionProfile = serde_yaml_ng::from_str(INIT_TEMPLATE).unwrap();
        assert_eq!(p.allow.len(), 3);
        assert!(p.deny.is_empty());
    }
}
