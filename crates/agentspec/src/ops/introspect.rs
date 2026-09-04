//! Discovery surfaces: what tools agentspec sees, and what it can be asked to do.
//!
//! Both are aimed as much at agents as at people — `--format json` gives an
//! agent the full command tree without shelling out to `--help` on every
//! subcommand, and the tool table answers "why isn't agentspec touching X".

use clap::Command as ClapCommand;
use console::style;
use serde_json::{Value, json};

use crate::error::Result;
use crate::tools;

/// Every supported tool, whether it is installed, and where it keeps things.
pub fn list_tools(json: bool) -> Result<()> {
    let rows: Vec<Value> = tools::all_tools()
        .iter()
        .map(|t| {
            let installed = t.is_installed();
            let mcp = t.mcp_target();
            json!({
                "slug": t.slug(),
                "name": t.name(),
                "installed": installed,
                "skills_dir": t.skills_dir().map(|p| p.to_string_lossy().to_string()),
                "agents_dir": t.agents_dir().map(|p| p.to_string_lossy().to_string()),
                "settings_path": t.settings_path().map(|p| p.to_string_lossy().to_string()),
                "mcp": mcp.as_ref().map(|m| json!({
                    "path": m.path.to_string_lossy(),
                    "dialect": m.dialect.label(),
                    "key": m.dialect.key(),
                })),
                "linked_skills": if installed { t.linked_skills().len() } else { 0 },
                "linked_agents": if installed { t.linked_agents().len() } else { 0 },
            })
        })
        .collect();

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "installed": rows.iter().filter(|r| r["installed"] == true).count(),
                "total": rows.len(),
                "tools": rows,
            }))?
        );
        return Ok(());
    }

    println!("    {:<16} {:<18} {:<14} SKILLS DIR", "SLUG", "NAME", "MCP");
    for r in &rows {
        let installed = r["installed"].as_bool().unwrap_or(false);
        let mark = if installed {
            style("✓").green()
        } else {
            style("-").dim()
        };
        let mcp = r["mcp"]["dialect"].as_str().unwrap_or("—");
        println!(
            "{:<3} {:<16} {:<18} {:<14} {}",
            mark.to_string(),
            r["slug"].as_str().unwrap_or(""),
            r["name"].as_str().unwrap_or(""),
            mcp,
            r["skills_dir"].as_str().unwrap_or("—"),
        );
    }
    let installed = rows.iter().filter(|r| r["installed"] == true).count();
    println!("\n{installed} of {} tool(s) installed", rows.len());
    Ok(())
}

/// The whole command tree. Handy for an agent that wants the surface area in
/// one call rather than recursively invoking `--help`.
pub fn print_commands(cmd: &ClapCommand, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(&describe(cmd, true))?);
        return Ok(());
    }
    println!("{}", style("agentspec").bold());
    print_tree(cmd, 1);
    println!(
        "\n{}",
        style("Run `agentspec <command> --help` for flags, or add --format json here for the full tree.").dim()
    );
    Ok(())
}

fn print_tree(cmd: &ClapCommand, depth: usize) {
    for sub in cmd.get_subcommands().filter(|s| !s.is_hide_set()) {
        let indent = "  ".repeat(depth);
        let about = sub.get_about().map(|a| a.to_string()).unwrap_or_default();
        println!("{indent}{:<14} {}", sub.get_name(), style(about).dim());
        print_tree(sub, depth + 1);
    }
}

fn describe(cmd: &ClapCommand, root: bool) -> Value {
    let args: Vec<Value> = cmd
        .get_arguments()
        .filter(|a| !a.is_hide_set())
        .map(|a| {
            json!({
                "id": a.get_id().as_str(),
                "long": a.get_long(),
                "short": a.get_short().map(|c| c.to_string()),
                "help": a.get_help().map(|h| h.to_string()),
                "required": a.is_required_set(),
                "takes_value": a.get_num_args().is_none_or(|n| n.takes_values()),
                "positional": a.is_positional(),
                "possible_values": a
                    .get_possible_values()
                    .iter()
                    .map(|p| p.get_name().to_string())
                    .collect::<Vec<_>>(),
                "default": a
                    .get_default_values()
                    .iter()
                    .map(|v| v.to_string_lossy().to_string())
                    .collect::<Vec<_>>(),
            })
        })
        .collect();

    let subcommands: Vec<Value> = cmd
        .get_subcommands()
        .filter(|s| !s.is_hide_set())
        .map(|s| describe(s, false))
        .collect();

    let mut out = json!({
        "name": cmd.get_name(),
        "about": cmd.get_about().map(|a| a.to_string()),
        "args": args,
        "subcommands": subcommands,
    });
    if root {
        out["long_about"] = json!(cmd.get_long_about().map(|a| a.to_string()));
        out["version"] = json!(env!("CARGO_PKG_VERSION"));
    }
    out
}
