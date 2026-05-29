//! MCP (Model Context Protocol) server management.
//!
//! agentspec keeps a canonical store of MCP server definitions in
//! `~/.agents/mcp/<name>.json` (one file per server, in the same shape tools
//! use under their `mcpServers` key). Servers can be defined once and injected
//! ("linked") into each tool's native settings:
//!
//! - Claude Code: `mcpServers` in `~/.claude/settings.json`
//! - Gemini CLI: `mcpServers` in `~/.gemini/settings.json`
//! - Cursor: `mcpServers` in `~/.cursor/mcp.json`
//!
//! `sync` also auto-discovers project `.mcp.json` files and registers them.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use console::style;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config;
use crate::error::{AppError, Result};
use crate::jsonfile::{read_json, write_json};
use crate::tools;

/// Canonical MCP server definition. Serialized verbatim into each tool's
/// `mcpServers.<name>` entry.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct McpServer {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub env: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub server_type: Option<String>,
}

impl McpServer {
    /// A server must be either a stdio server (command) or a remote (url).
    pub fn validate(&self) -> Result<()> {
        match (self.command.is_some(), self.url.is_some()) {
            (true, true) => Err(AppError::Other(
                "an MCP server cannot set both --command and --url".into(),
            )),
            (false, false) => Err(AppError::Other(
                "an MCP server needs --command (stdio) or --url (http/sse)".into(),
            )),
            _ => Ok(()),
        }
    }

    fn to_json(&self) -> Value {
        serde_json::to_value(self).unwrap_or_else(|_| serde_json::json!({}))
    }
}

/// Get installed tools that have MCP config support, as (slug, config path).
fn mcp_targets() -> Vec<(String, PathBuf)> {
    tools::installed_tools()
        .into_iter()
        .filter_map(|t| {
            let path = t.mcp_config_path()?;
            Some((t.slug().to_string(), path))
        })
        .collect()
}

fn store_path(name: &str) -> PathBuf {
    config::shared_mcp_dir().join(format!("{name}.json"))
}

/// Inject a server definition into one tool's `mcpServers` map.
fn inject(path: &Path, name: &str, server: &Value) -> Result<()> {
    let mut root = read_json(path);
    if !root.is_object() {
        root = serde_json::json!({});
    }
    let servers = root
        .as_object_mut()
        .unwrap()
        .entry("mcpServers")
        .or_insert_with(|| serde_json::json!({}));
    // A hand-edited config may have a non-object `mcpServers`; normalize it.
    if !servers.is_object() {
        *servers = serde_json::json!({});
    }
    servers
        .as_object_mut()
        .unwrap()
        .insert(name.to_string(), server.clone());
    write_json(path, &root)
}

/// Resolve the fan-out targets given an optional tool filter.
fn resolve_targets(tool: Option<&str>) -> Result<Vec<(String, PathBuf)>> {
    match tool {
        Some(t) => {
            let filtered: Vec<_> = mcp_targets()
                .into_iter()
                .filter(|(slug, _)| slug == t)
                .collect();
            if filtered.is_empty() {
                return Err(AppError::Other(format!(
                    "tool '{t}' not found or has no MCP config support (try claude-code, gemini-cli, cursor)"
                )));
            }
            Ok(filtered)
        }
        None => Ok(mcp_targets()),
    }
}

/// Register a server: write to the canonical store, then inject into tools.
pub fn add_server(tool: Option<&str>, name: &str, server: &McpServer) -> Result<()> {
    server.validate()?;
    let json = server.to_json();

    // Validate the target filter BEFORE mutating the store, so a bad --tool
    // doesn't leave a half-applied registration behind.
    let targets = resolve_targets(tool)?;

    // Canonical store is always authoritative.
    std::fs::create_dir_all(config::shared_mcp_dir())?;
    write_json(&store_path(name), &json)?;
    eprintln!("  stored {name} in canonical store");

    if targets.is_empty() {
        eprintln!(
            "  {} no installed tools with MCP support; server kept in store only",
            style("~").yellow()
        );
        return Ok(());
    }
    for (slug, path) in &targets {
        inject(path, name, &json)?;
        eprintln!("  registered {name} in {slug}");
    }
    Ok(())
}

/// Inject a previously-stored server into tool config(s).
pub fn link_server(tool: Option<&str>, name: &str) -> Result<()> {
    let path = store_path(name);
    if !path.exists() {
        return Err(AppError::Other(format!(
            "server '{name}' not found in canonical store ({})",
            config::shared_mcp_dir().display()
        )));
    }
    let json = read_json(&path);
    let targets = resolve_targets(tool)?;
    if targets.is_empty() {
        return Err(AppError::Other(
            "no installed tools with MCP config support found".into(),
        ));
    }
    for (slug, config_path) in &targets {
        inject(config_path, name, &json)?;
        eprintln!("  linked {name} → {slug}");
    }
    Ok(())
}

/// Link every server in the canonical store to all MCP-capable tools.
pub fn sync_all_servers() -> Result<()> {
    let stored = list_stored();
    if stored.is_empty() {
        println!(
            "  no servers in canonical store ({})",
            config::shared_mcp_dir().display()
        );
        return Ok(());
    }
    let targets = mcp_targets();
    if targets.is_empty() {
        println!("  no installed tools with MCP config support found");
        return Ok(());
    }
    let mut count = 0;
    for (name, json) in &stored {
        for (slug, path) in &targets {
            inject(path, name, json)?;
            eprintln!("  {name} → {slug}");
            count += 1;
        }
    }
    println!(
        "  synced {} canonical server(s) to {} tool(s)",
        stored.len(),
        targets.len()
    );
    let _ = count;
    Ok(())
}

/// Remove a server from tool configs, and optionally the canonical store.
pub fn remove_server(tool: Option<&str>, name: &str, purge: bool) -> Result<()> {
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

    if purge || tool.is_none() {
        let sp = store_path(name);
        if sp.exists() {
            std::fs::remove_file(&sp)?;
            eprintln!("  removed {name} from canonical store");
        }
    }

    Ok(())
}

/// Read all servers from the canonical store, skipping unparseable files.
fn list_stored() -> Vec<(String, Value)> {
    let dir = config::shared_mcp_dir();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let name = path.file_stem().unwrap().to_string_lossy().to_string();
        match std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        {
            Some(v) => out.push((name, v)),
            None => eprintln!(
                "  {} skipping unparseable {}",
                style("~").yellow(),
                path.display()
            ),
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

fn server_summary(config: &Value) -> String {
    if let Some(url) = config.get("url").and_then(|v| v.as_str()) {
        return url.to_string();
    }
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
    format!("{cmd} {args}").trim().to_string()
}

/// List the canonical store and each tool's registered MCP servers.
pub fn list_servers() -> Result<()> {
    let stored = list_stored();
    println!("{}", style("canonical store (~/.agents/mcp):").bold());
    if stored.is_empty() {
        println!("  (none)");
    } else {
        for (name, config) in &stored {
            println!("  {name}: {}", server_summary(config));
        }
    }

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
                println!("  {name}: {}", server_summary(config));
            }
        }
    }
    Ok(())
}

/// Discover `.mcp.json` files in project roots and auto-register servers
/// in all installed tools. Called during `agentspec sync`.
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

        let parsed: Value = match serde_json::from_str(&content) {
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
                if !tool_root.is_object() {
                    tool_root = serde_json::json!({});
                }
                let tool_servers = tool_root
                    .as_object_mut()
                    .unwrap()
                    .entry("mcpServers")
                    .or_insert_with(|| serde_json::json!({}));
                if !tool_servers.is_object() {
                    *tool_servers = serde_json::json!({});
                }

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

    // Claude Code project memory. Decoding the encoded dir name is lossy
    // (hyphens and path separators share a character), so reuse the
    // filesystem-aware greedy decoder rather than a naive replace.
    if let Some(home) = dirs::home_dir() {
        let projects_dir = home.join(".claude").join("projects");
        if let Ok(entries) = std::fs::read_dir(&projects_dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let encoded = entry.file_name().to_string_lossy().to_string();
                if let Some(path) = crate::ops::memory::decode_project_path(&encoded)
                    && path.join(".git").exists()
                {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_requires_command_xor_url() {
        let mut s = McpServer::default();
        assert!(s.validate().is_err()); // neither
        s.command = Some("sr".into());
        assert!(s.validate().is_ok());
        s.url = Some("https://x".into());
        assert!(s.validate().is_err()); // both
        s.command = None;
        assert!(s.validate().is_ok()); // url only
    }

    #[test]
    fn stdio_server_json_shape() {
        let s = McpServer {
            command: Some("sr".into()),
            args: vec!["mcp".into(), "serve".into()],
            env: HashMap::from([("API_KEY".to_string(), "x".to_string())]),
            url: None,
            server_type: None,
        };
        let j = s.to_json();
        assert_eq!(j.get("command").unwrap(), "sr");
        assert_eq!(j.get("args").unwrap(), &serde_json::json!(["mcp", "serve"]));
        assert_eq!(j.get("env").unwrap().get("API_KEY").unwrap(), "x");
        assert!(j.get("url").is_none());
    }

    #[test]
    fn http_server_json_shape() {
        let s = McpServer {
            command: None,
            args: vec![],
            env: HashMap::new(),
            url: Some("https://mcp.example/sse".into()),
            server_type: Some("http".into()),
        };
        let j = s.to_json();
        assert_eq!(j.get("url").unwrap(), "https://mcp.example/sse");
        assert_eq!(j.get("type").unwrap(), "http");
        assert!(j.get("command").is_none());
        assert!(j.get("args").is_none()); // empty args skipped
    }

    #[test]
    fn summary_prefers_url_then_command() {
        assert_eq!(
            server_summary(&serde_json::json!({"url":"https://x"})),
            "https://x"
        );
        assert_eq!(
            server_summary(&serde_json::json!({"command":"sr","args":["mcp","serve"]})),
            "sr mcp serve"
        );
    }
}
