use crate::error::{AppError, Result};
use crate::tools;
use std::path::PathBuf;

/// Read a JSON file, returning empty object if missing or invalid.
fn read_json(path: &PathBuf) -> serde_json::Value {
    if !path.exists() {
        return serde_json::json!({});
    }
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(serde_json::json!({}))
}

/// Write a JSON value to a file, creating parent dirs as needed.
fn write_json(path: &PathBuf, val: &serde_json::Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(val)?;
    std::fs::write(path, content)?;
    Ok(())
}

/// Get installed tools that have MCP config support.
fn mcp_targets() -> Vec<(String, PathBuf)> {
    tools::installed_tools()
        .into_iter()
        .filter_map(|t| {
            let path = t.mcp_config_path()?;
            Some((t.slug().to_string(), path))
        })
        .collect()
}

/// Register an MCP server in tool configs.
/// If `tool` is None, registers in all installed tools with MCP support.
pub fn add_server(tool: Option<&str>, name: &str, command: &str, args: &[String]) -> Result<()> {
    let server = serde_json::json!({
        "command": command,
        "args": args,
    });

    let targets = match tool {
        Some(t) => {
            let all = mcp_targets();
            let filtered: Vec<_> = all.into_iter().filter(|(slug, _)| slug == t).collect();
            if filtered.is_empty() {
                return Err(AppError::Other(format!(
                    "tool '{t}' not found or has no MCP config support"
                )));
            }
            filtered
        }
        None => mcp_targets(),
    };

    if targets.is_empty() {
        return Err(AppError::Other(
            "no installed tools with MCP config support found".into(),
        ));
    }

    for (slug, path) in &targets {
        let mut root = read_json(path);
        let servers = root
            .as_object_mut()
            .unwrap()
            .entry("mcpServers")
            .or_insert_with(|| serde_json::json!({}));
        servers
            .as_object_mut()
            .unwrap()
            .insert(name.to_string(), server.clone());
        write_json(path, &root)?;
        eprintln!("  registered {name} in {slug}");
    }

    Ok(())
}

/// Remove an MCP server from tool configs.
pub fn remove_server(tool: Option<&str>, name: &str) -> Result<()> {
    let targets = match tool {
        Some(t) => mcp_targets()
            .into_iter()
            .filter(|(slug, _)| slug == t)
            .collect(),
        None => mcp_targets(),
    };

    for (slug, path) in &targets {
        if !path.exists() {
            continue;
        }
        let mut root = read_json(path);
        if let Some(servers) = root.get_mut("mcpServers").and_then(|v| v.as_object_mut())
            && servers.remove(name).is_some()
        {
            write_json(path, &root)?;
            eprintln!("  removed {name} from {slug}");
        }
    }

    Ok(())
}

/// List registered MCP servers across all installed tools.
pub fn list_servers() -> Result<()> {
    let mut found_any = false;
    for (slug, path) in mcp_targets() {
        if !path.exists() {
            continue;
        }
        let root = read_json(&path);
        if let Some(servers) = root.get("mcpServers").and_then(|v| v.as_object())
            && !servers.is_empty()
        {
            println!("{slug}:");
            for (name, config) in servers {
                let cmd = config
                    .get("command")
                    .and_then(|v| v.as_str())
                    .unwrap_or("?");
                let args = config
                    .get("args")
                    .and_then(|v| v.as_array())
                    .map(|a| {
                        a.iter()
                            .filter_map(|v| v.as_str())
                            .collect::<Vec<_>>()
                            .join(" ")
                    })
                    .unwrap_or_default();
                println!("  {name}: {cmd} {args}");
                found_any = true;
            }
        }
    }
    if !found_any {
        println!("no MCP servers registered");
    }
    Ok(())
}

/// Discover `.mcp.json` files in project roots and auto-register servers
/// in all installed tools. Called during `agentspec sync`.
///
/// `.mcp.json` format (emerging standard across Cursor, VS Code, Claude Code):
/// ```json
/// {
///   "mcpServers": {
///     "name": { "command": "...", "args": [...], "env": {...} }
///   }
/// }
/// ```
pub fn discover_and_register(project_roots: &[PathBuf]) -> Result<()> {
    let targets = mcp_targets();
    if targets.is_empty() {
        return Ok(());
    }

    let mut registered = 0;

    for root in project_roots {
        let mcp_path = root.join(".mcp.json");
        if !mcp_path.exists() {
            continue;
        }

        let content = match std::fs::read_to_string(&mcp_path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let parsed: serde_json::Value = match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("  warning: invalid .mcp.json in {}: {e}", root.display());
                continue;
            }
        };

        let servers = match parsed.get("mcpServers").and_then(|v| v.as_object()) {
            Some(s) => s,
            None => continue,
        };

        for (name, server_config) in servers {
            for (slug, config_path) in &targets {
                let mut tool_root = read_json(config_path);
                let tool_servers = tool_root
                    .as_object_mut()
                    .unwrap()
                    .entry("mcpServers")
                    .or_insert_with(|| serde_json::json!({}));

                if tool_servers.get(name).is_none() {
                    tool_servers
                        .as_object_mut()
                        .unwrap()
                        .insert(name.clone(), server_config.clone());
                    write_json(config_path, &tool_root)?;
                    eprintln!("  discovered {name} → {slug}");
                    registered += 1;
                }
            }
        }
    }

    if registered > 0 {
        eprintln!("  registered {registered} MCP server(s) from .mcp.json");
    }

    Ok(())
}

/// Collect project roots for MCP discovery.
/// Sources: Claude Code project memory + current working directory.
pub fn collect_project_roots() -> Vec<PathBuf> {
    let mut roots = vec![];

    // Claude Code project memory
    if let Some(home) = dirs::home_dir() {
        let projects_dir = home.join(".claude").join("projects");
        if let Ok(entries) = std::fs::read_dir(&projects_dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let encoded = entry.file_name().to_string_lossy().to_string();
                let decoded = encoded.replacen('-', "/", 1).replace('-', "/");
                let path = PathBuf::from(&decoded);
                if path.exists() && path.join(".git").exists() {
                    roots.push(path);
                }
            }
        }
    }

    // Current directory
    if let Ok(cwd) = std::env::current_dir()
        && cwd.join(".mcp.json").exists()
        && !roots.contains(&cwd)
    {
        roots.push(cwd);
    }

    roots
}
