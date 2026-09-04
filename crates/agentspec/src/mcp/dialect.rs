//! Translation between the canonical MCP server shape and each tool's native
//! dialect.
//!
//! agentspec stores one canonical definition per server in `~/.agents/mcp/`
//! using the widely-shared `mcpServers` shape (`command`/`args`/`env`, or
//! `url`/`type`). Tools that speak a different dialect — Codex's TOML tables,
//! OpenCode's `mcp` key with an argv array — are translated on read and write
//! so a server registered once appears natively everywhere.

use std::path::Path;

use serde_json::{Map, Value};
use toml_edit::{Array, DocumentMut, InlineTable, Item, Table, value as toml_value};

use crate::error::Result;
use crate::jsonfile::{read_json, write_json};
use crate::tools::{McpDialect, McpTarget};

/// Read every server a tool currently has registered, in canonical shape.
pub fn read_servers(target: &McpTarget) -> Vec<(String, Value)> {
    let mut out: Vec<(String, Value)> = match target.dialect {
        McpDialect::JsonMap(key) => read_json_map(&target.path, key),
        McpDialect::OpenCodeJson(key) => read_json_map(&target.path, key)
            .into_iter()
            .map(|(n, v)| (n, from_opencode(&v)))
            .collect(),
        McpDialect::TomlTable(table) => read_toml_table(&target.path, table),
    };
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// Write one canonical server into a tool's config. Returns whether the file
/// actually changed — an already-in-sync entry is a no-op, so `sync` stays quiet.
pub fn write_server(target: &McpTarget, name: &str, canonical: &Value) -> Result<bool> {
    match target.dialect {
        McpDialect::JsonMap(key) => write_json_map(&target.path, key, name, canonical),
        McpDialect::OpenCodeJson(key) => {
            write_json_map(&target.path, key, name, &to_opencode(canonical))
        }
        McpDialect::TomlTable(table) => write_toml_table(&target.path, table, name, canonical),
    }
}

/// Remove a server from a tool's config. Returns whether it was present.
pub fn remove_server(target: &McpTarget, name: &str) -> Result<bool> {
    if !target.path.exists() {
        return Ok(false);
    }
    match target.dialect {
        McpDialect::JsonMap(key) | McpDialect::OpenCodeJson(key) => {
            remove_json_map(&target.path, key, name)
        }
        McpDialect::TomlTable(table) => remove_toml_table(&target.path, table, name),
    }
}

// ---------------------------------------------------------------------------
// JSON map dialects (Claude Code, Gemini CLI, Cursor, Copilot, Windsurf, Cline, Amp)
// ---------------------------------------------------------------------------

// The key is a literal top-level key, never a dotted path: Amp stores its
// servers under the single VS Code-style key `"amp.mcpServers"`.
fn read_json_map(path: &Path, key: &str) -> Vec<(String, Value)> {
    if !path.exists() {
        return Vec::new();
    }
    read_json(path)
        .get(key)
        .and_then(|v| v.as_object().cloned())
        .map(|m| m.into_iter().collect())
        .unwrap_or_default()
}

fn write_json_map(path: &Path, key: &str, name: &str, entry: &Value) -> Result<bool> {
    let mut root = read_json(path);
    if !root.is_object() {
        root = Value::Object(Map::new());
    }
    // A hand-edited config may have a non-object at the key; normalize it.
    let mut servers = root
        .get(key)
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default();
    if servers.get(name) == Some(entry) {
        return Ok(false);
    }
    servers.insert(name.to_string(), entry.clone());
    root.as_object_mut()
        .unwrap()
        .insert(key.to_string(), Value::Object(servers));
    write_json(path, &root)?;
    Ok(true)
}

fn remove_json_map(path: &Path, key: &str, name: &str) -> Result<bool> {
    let mut root = read_json(path);
    let Some(mut servers) = root.get(key).and_then(|v| v.as_object().cloned()) else {
        return Ok(false);
    };
    if servers.remove(name).is_none() {
        return Ok(false);
    }
    root.as_object_mut()
        .unwrap()
        .insert(key.to_string(), Value::Object(servers));
    write_json(path, &root)?;
    Ok(true)
}

// ---------------------------------------------------------------------------
// OpenCode dialect: {type: local|remote, command: [argv…], environment, enabled}
// ---------------------------------------------------------------------------

fn to_opencode(canonical: &Value) -> Value {
    if let Some(url) = canonical.get("url").and_then(|v| v.as_str()) {
        return serde_json::json!({ "type": "remote", "url": url, "enabled": true });
    }
    let mut argv: Vec<Value> = Vec::new();
    if let Some(cmd) = canonical.get("command").and_then(|v| v.as_str()) {
        argv.push(Value::String(cmd.to_string()));
    }
    if let Some(args) = canonical.get("args").and_then(|v| v.as_array()) {
        argv.extend(args.iter().cloned());
    }
    let mut out = serde_json::json!({
        "type": "local",
        "command": argv,
        "enabled": true,
    });
    if let Some(env) = canonical.get("env").and_then(|v| v.as_object())
        && !env.is_empty()
    {
        out["environment"] = Value::Object(env.clone());
    }
    out
}

fn from_opencode(entry: &Value) -> Value {
    if entry.get("type").and_then(|v| v.as_str()) == Some("remote") || entry.get("url").is_some() {
        let mut out = Map::new();
        if let Some(url) = entry.get("url") {
            out.insert("url".into(), url.clone());
        }
        out.insert("type".into(), Value::String("http".into()));
        return Value::Object(out);
    }
    let argv: Vec<Value> = entry
        .get("command")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let mut out = Map::new();
    if let Some(first) = argv.first() {
        out.insert("command".into(), first.clone());
    }
    if argv.len() > 1 {
        out.insert("args".into(), Value::Array(argv[1..].to_vec()));
    }
    if let Some(env) = entry.get("environment").and_then(|v| v.as_object())
        && !env.is_empty()
    {
        out.insert("env".into(), Value::Object(env.clone()));
    }
    Value::Object(out)
}

// ---------------------------------------------------------------------------
// TOML dialect (Codex `[mcp_servers.<name>]`)
//
// Edited through toml_edit so surrounding comments and hand formatting in a
// user's config.toml survive a write.
// ---------------------------------------------------------------------------

fn read_toml_doc(path: &Path) -> DocumentMut {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| s.parse::<DocumentMut>().ok())
        .unwrap_or_default()
}

fn read_toml_table(path: &Path, table: &str) -> Vec<(String, Value)> {
    if !path.exists() {
        return Vec::new();
    }
    let doc = read_toml_doc(path);
    let Some(servers) = doc.get(table).and_then(|i| i.as_table_like()) else {
        return Vec::new();
    };
    servers
        .iter()
        .filter_map(|(name, item)| Some((name.to_string(), toml_item_to_canonical(item)?)))
        .collect()
}

fn toml_item_to_canonical(item: &Item) -> Option<Value> {
    let t = item.as_table_like()?;
    let mut out = Map::new();
    if let Some(cmd) = t.get("command").and_then(|v| v.as_str()) {
        out.insert("command".into(), Value::String(cmd.to_string()));
    }
    if let Some(url) = t.get("url").and_then(|v| v.as_str()) {
        out.insert("url".into(), Value::String(url.to_string()));
        out.insert("type".into(), Value::String("http".into()));
    }
    if let Some(args) = t.get("args").and_then(|v| v.as_array()) {
        let args: Vec<Value> = args
            .iter()
            .filter_map(|a| a.as_str().map(|s| Value::String(s.to_string())))
            .collect();
        if !args.is_empty() {
            out.insert("args".into(), Value::Array(args));
        }
    }
    if let Some(env) = t.get("env").and_then(|v| v.as_table_like()) {
        let env: Map<String, Value> = env
            .iter()
            .filter_map(|(k, v)| Some((k.to_string(), Value::String(v.as_str()?.to_string()))))
            .collect();
        if !env.is_empty() {
            out.insert("env".into(), Value::Object(env));
        }
    }
    (!out.is_empty()).then_some(Value::Object(out))
}

fn canonical_to_toml_table(canonical: &Value) -> Table {
    let mut t = Table::new();
    if let Some(cmd) = canonical.get("command").and_then(|v| v.as_str()) {
        t.insert("command", toml_value(cmd));
    }
    if let Some(url) = canonical.get("url").and_then(|v| v.as_str()) {
        t.insert("url", toml_value(url));
    }
    if let Some(args) = canonical.get("args").and_then(|v| v.as_array())
        && !args.is_empty()
    {
        let mut arr = Array::new();
        for a in args.iter().filter_map(|a| a.as_str()) {
            arr.push(a);
        }
        t.insert("args", toml_value(arr));
    }
    if let Some(env) = canonical.get("env").and_then(|v| v.as_object())
        && !env.is_empty()
    {
        let mut it = InlineTable::new();
        for (k, v) in env {
            if let Some(s) = v.as_str() {
                it.insert(k, s.into());
            }
        }
        t.insert("env", toml_value(it));
    }
    t
}

fn write_toml_table(path: &Path, table: &str, name: &str, canonical: &Value) -> Result<bool> {
    let mut doc = read_toml_doc(path);
    // Compare against what is already there so an in-sync entry is a no-op.
    if let Some(existing) = doc
        .get(table)
        .and_then(|i| i.as_table_like())
        .and_then(|t| t.get(name))
        .and_then(toml_item_to_canonical)
        && &existing == canonical
    {
        return Ok(false);
    }
    let servers = doc
        .entry(table)
        .or_insert_with(|| Item::Table(implicit_table()));
    if servers.as_table_like().is_none() {
        *servers = Item::Table(implicit_table());
    }
    let Some(servers) = servers.as_table_like_mut() else {
        return Ok(false);
    };
    servers.insert(name, Item::Table(canonical_to_toml_table(canonical)));
    write_toml(path, &doc)?;
    Ok(true)
}

fn remove_toml_table(path: &Path, table: &str, name: &str) -> Result<bool> {
    let mut doc = read_toml_doc(path);
    let Some(servers) = doc.get_mut(table).and_then(|i| i.as_table_like_mut()) else {
        return Ok(false);
    };
    if servers.remove(name).is_none() {
        return Ok(false);
    }
    write_toml(path, &doc)?;
    Ok(true)
}

/// A parent table written as `[mcp_servers.name]` headers rather than an
/// empty `[mcp_servers]` stanza.
fn implicit_table() -> Table {
    let mut t = Table::new();
    t.set_implicit(true);
    t
}

fn write_toml(path: &Path, doc: &DocumentMut) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, doc.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stdio() -> Value {
        serde_json::json!({
            "command": "sr",
            "args": ["mcp", "serve"],
            "env": {"API_KEY": "x"}
        })
    }

    #[test]
    fn opencode_stdio_round_trip() {
        let oc = to_opencode(&stdio());
        assert_eq!(oc["type"], "local");
        assert_eq!(oc["command"], serde_json::json!(["sr", "mcp", "serve"]));
        assert_eq!(oc["environment"]["API_KEY"], "x");
        assert_eq!(from_opencode(&oc), stdio());
    }

    #[test]
    fn opencode_remote_round_trip() {
        let remote = serde_json::json!({"url": "https://mcp.example/sse", "type": "http"});
        let oc = to_opencode(&remote);
        assert_eq!(oc["type"], "remote");
        assert_eq!(from_opencode(&oc), remote);
    }

    #[test]
    fn toml_round_trip_preserves_comments() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.toml");
        std::fs::write(&path, "# keep me\nmodel = \"gpt\"\n").unwrap();

        assert!(write_toml_table(&path, "mcp_servers", "sr", &stdio()).unwrap());
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("# keep me"), "comment dropped:\n{text}");
        assert!(text.contains("[mcp_servers.sr]"), "missing table:\n{text}");

        let read = read_toml_table(&path, "mcp_servers");
        assert_eq!(read, vec![("sr".to_string(), stdio())]);

        // Writing the same definition again is a no-op.
        assert!(!write_toml_table(&path, "mcp_servers", "sr", &stdio()).unwrap());

        assert!(remove_toml_table(&path, "mcp_servers", "sr").unwrap());
        assert!(read_toml_table(&path, "mcp_servers").is_empty());
        assert!(!remove_toml_table(&path, "mcp_servers", "sr").unwrap());
    }

    #[test]
    fn json_map_treats_key_as_literal() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("settings.json");
        std::fs::write(&path, r#"{"amp.theme":"dark"}"#).unwrap();

        assert!(write_json_map(&path, "amp.mcpServers", "sr", &stdio()).unwrap());
        let root = read_json(&path);
        assert_eq!(root["amp.theme"], "dark");
        assert_eq!(
            read_json_map(&path, "amp.mcpServers"),
            vec![("sr".to_string(), stdio())]
        );
        assert!(!write_json_map(&path, "amp.mcpServers", "sr", &stdio()).unwrap());
        assert!(remove_json_map(&path, "amp.mcpServers", "sr").unwrap());
    }
}
