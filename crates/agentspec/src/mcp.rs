use crate::error::{AppError, Result};
use std::path::PathBuf;

/// Known AI tool config paths that support mcpServers.
fn tool_configs() -> Vec<(&'static str, PathBuf)> {
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return vec![],
    };

    vec![
        ("claude-code", home.join(".claude").join("settings.json")),
        ("cursor", home.join(".cursor").join("mcp.json")),
    ]
}

fn read_json(path: &PathBuf) -> serde_json::Value {
    if !path.exists() {
        return serde_json::json!({});
    }
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(serde_json::json!({}))
}

fn write_json(path: &PathBuf, val: &serde_json::Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(val)?;
    std::fs::write(path, content)?;
    Ok(())
}

pub fn add_server(
    tool: Option<&str>,
    name: &str,
    command: &str,
    args: &[String],
) -> Result<()> {
    let server = serde_json::json!({
        "command": command,
        "args": args,
    });

    let configs = tool_configs();
    let targets: Vec<_> = match tool {
        Some(t) => configs.into_iter().filter(|(slug, _)| *slug == t).collect(),
        None => configs,
    };

    if targets.is_empty() {
        return Err(AppError::Other(format!(
            "no matching tool found{}",
            tool.map(|t| format!(" for '{t}'")).unwrap_or_default()
        )));
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
        eprintln!("  registered {name} in {slug} ({})", path.display());
    }

    Ok(())
}

pub fn remove_server(tool: Option<&str>, name: &str) -> Result<()> {
    let configs = tool_configs();
    let targets: Vec<_> = match tool {
        Some(t) => configs.into_iter().filter(|(slug, _)| *slug == t).collect(),
        None => configs,
    };

    for (slug, path) in &targets {
        if !path.exists() {
            continue;
        }
        let mut root = read_json(path);
        if let Some(servers) = root.get_mut("mcpServers").and_then(|v| v.as_object_mut()) {
            if servers.remove(name).is_some() {
                write_json(path, &root)?;
                eprintln!("  removed {name} from {slug}");
            }
        }
    }

    Ok(())
}

pub fn list_servers() -> Result<()> {
    let mut found_any = false;
    for (slug, path) in tool_configs() {
        if !path.exists() {
            continue;
        }
        let root = read_json(&path);
        if let Some(servers) = root.get("mcpServers").and_then(|v| v.as_object()) {
            if !servers.is_empty() {
                println!("{slug}:");
                for (name, config) in servers {
                    let cmd = config.get("command").and_then(|v| v.as_str()).unwrap_or("?");
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
    }
    if !found_any {
        println!("no MCP servers registered");
    }
    Ok(())
}
