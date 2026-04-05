use crate::error::{AppError, Result};
use crate::tools;
use std::path::PathBuf;

/// Trait for reading/writing a tool's native settings format.
pub trait ToolSettings: Send + Sync {
    fn tool_slug(&self) -> &str;
    fn settings_path(&self) -> Option<PathBuf>;
    fn read_raw(&self) -> Result<serde_json::Value>;
    fn get(&self, key: &str) -> Result<Option<serde_json::Value>>;
    fn set(&self, key: &str, value: serde_json::Value) -> Result<()>;
    fn keys(&self) -> Result<Vec<String>>;
}

/// JSON-based settings (Claude, Gemini, Copilot, Cline).
struct JsonSettings {
    slug: &'static str,
}

impl JsonSettings {
    fn path(&self) -> Option<PathBuf> {
        tools::find_tool(self.slug).and_then(|t| t.config_path())
    }
}

impl ToolSettings for JsonSettings {
    fn tool_slug(&self) -> &str {
        self.slug
    }

    fn settings_path(&self) -> Option<PathBuf> {
        self.path()
    }

    fn read_raw(&self) -> Result<serde_json::Value> {
        let path = self
            .path()
            .ok_or_else(|| AppError::Other(format!("No config path for {}", self.slug)))?;
        if !path.exists() {
            return Ok(serde_json::Value::Object(Default::default()));
        }
        let content = std::fs::read_to_string(&path)?;
        let val: serde_json::Value = serde_json::from_str(&content)?;
        Ok(val)
    }

    fn get(&self, key: &str) -> Result<Option<serde_json::Value>> {
        let root = self.read_raw()?;
        Ok(get_dotted(&root, key))
    }

    fn set(&self, key: &str, value: serde_json::Value) -> Result<()> {
        let path = self
            .path()
            .ok_or_else(|| AppError::Other(format!("No config path for {}", self.slug)))?;
        let mut root = self.read_raw()?;
        set_dotted(&mut root, key, value);
        let content = serde_json::to_string_pretty(&root)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, content)?;
        Ok(())
    }

    fn keys(&self) -> Result<Vec<String>> {
        let root = self.read_raw()?;
        Ok(collect_keys(&root, ""))
    }
}

/// TOML-based settings (Codex).
struct TomlSettings;

impl ToolSettings for TomlSettings {
    fn tool_slug(&self) -> &str {
        "codex"
    }

    fn settings_path(&self) -> Option<PathBuf> {
        tools::find_tool("codex").and_then(|t| t.config_path())
    }

    fn read_raw(&self) -> Result<serde_json::Value> {
        let path = self
            .settings_path()
            .ok_or_else(|| AppError::Other("No config path for codex".into()))?;
        if !path.exists() {
            return Ok(serde_json::Value::Object(Default::default()));
        }
        let content = std::fs::read_to_string(&path)?;
        // Parse TOML as generic value, then convert to JSON
        let toml_val: toml::Value = toml::from_str(&content)
            .map_err(|e| AppError::Other(format!("TOML parse error: {e}")))?;
        let json = toml_to_json(&toml_val);
        Ok(json)
    }

    fn get(&self, key: &str) -> Result<Option<serde_json::Value>> {
        let root = self.read_raw()?;
        Ok(get_dotted(&root, key))
    }

    fn set(&self, key: &str, value: serde_json::Value) -> Result<()> {
        // For TOML, we read-modify-write via JSON then convert back
        let path = self
            .settings_path()
            .ok_or_else(|| AppError::Other("No config path for codex".into()))?;
        let mut root = self.read_raw()?;
        set_dotted(&mut root, key, value);
        let toml_val = json_to_toml(&root);
        let content = toml::to_string_pretty(&toml_val)
            .map_err(|e| AppError::Other(format!("TOML serialize error: {e}")))?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, content)?;
        Ok(())
    }

    fn keys(&self) -> Result<Vec<String>> {
        let root = self.read_raw()?;
        Ok(collect_keys(&root, ""))
    }
}

fn all_settings() -> Vec<Box<dyn ToolSettings>> {
    vec![
        Box::new(JsonSettings {
            slug: "claude-code",
        }),
        Box::new(TomlSettings),
        Box::new(JsonSettings { slug: "gemini-cli" }),
        Box::new(JsonSettings {
            slug: "github-copilot",
        }),
        Box::new(JsonSettings { slug: "cline" }),
    ]
}

fn settings_for_tool(slug: &str) -> Option<Box<dyn ToolSettings>> {
    let canonical = match slug {
        "claude" => "claude-code",
        _ => slug,
    };
    all_settings()
        .into_iter()
        .find(|s| s.tool_slug() == canonical)
}

// --- CLI entry points ---

pub fn list_all_settings() {
    for s in all_settings() {
        let path = s
            .settings_path()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "(not configured)".to_string());
        let exists = s.settings_path().is_some_and(|p| p.exists());
        let status = if exists { "exists" } else { "missing" };
        println!("{:<16} {} [{}]", s.tool_slug(), path, status);
    }
}

pub fn show_settings(tool: &str) -> Result<()> {
    let s = settings_for_tool(tool)
        .ok_or_else(|| AppError::Other(format!("No settings handler for: {tool}")))?;
    let raw = s.read_raw()?;
    println!("{}", serde_json::to_string_pretty(&raw)?);
    Ok(())
}

pub fn get_setting(tool: &str, key: &str) -> Result<()> {
    let s = settings_for_tool(tool)
        .ok_or_else(|| AppError::Other(format!("No settings handler for: {tool}")))?;
    match s.get(key)? {
        Some(val) => println!("{}", serde_json::to_string_pretty(&val)?),
        None => eprintln!("Key not found: {key}"),
    }
    Ok(())
}

pub fn set_setting(tool: &str, key: &str, value: &str) -> Result<()> {
    let s = settings_for_tool(tool)
        .ok_or_else(|| AppError::Other(format!("No settings handler for: {tool}")))?;
    let json_val: serde_json::Value = serde_json::from_str(value)
        .unwrap_or_else(|_| serde_json::Value::String(value.to_string()));
    s.set(key, json_val)?;
    println!("Set {key} for {tool}");
    Ok(())
}

pub fn list_keys(tool: &str) -> Result<()> {
    let s = settings_for_tool(tool)
        .ok_or_else(|| AppError::Other(format!("No settings handler for: {tool}")))?;
    for key in s.keys()? {
        println!("{key}");
    }
    Ok(())
}

// --- Helpers ---

fn get_dotted(val: &serde_json::Value, key: &str) -> Option<serde_json::Value> {
    let mut current = val;
    for part in key.split('.') {
        current = current.get(part)?;
    }
    Some(current.clone())
}

fn set_dotted(val: &mut serde_json::Value, key: &str, new_val: serde_json::Value) {
    let parts: Vec<&str> = key.split('.').collect();
    let mut current = val;
    for part in &parts[..parts.len() - 1] {
        current = current
            .as_object_mut()
            .unwrap()
            .entry(*part)
            .or_insert_with(|| serde_json::Value::Object(Default::default()));
    }
    if let Some(obj) = current.as_object_mut() {
        obj.insert(parts.last().unwrap().to_string(), new_val);
    }
}

fn collect_keys(val: &serde_json::Value, prefix: &str) -> Vec<String> {
    let mut keys = Vec::new();
    if let Some(obj) = val.as_object() {
        for (k, v) in obj {
            let full_key = if prefix.is_empty() {
                k.clone()
            } else {
                format!("{prefix}.{k}")
            };
            if v.is_object() {
                keys.extend(collect_keys(v, &full_key));
            } else {
                keys.push(full_key);
            }
        }
    }
    keys
}

fn toml_to_json(val: &toml::Value) -> serde_json::Value {
    match val {
        toml::Value::String(s) => serde_json::Value::String(s.clone()),
        toml::Value::Integer(i) => serde_json::json!(*i),
        toml::Value::Float(f) => serde_json::json!(*f),
        toml::Value::Boolean(b) => serde_json::Value::Bool(*b),
        toml::Value::Datetime(d) => serde_json::Value::String(d.to_string()),
        toml::Value::Array(a) => serde_json::Value::Array(a.iter().map(toml_to_json).collect()),
        toml::Value::Table(t) => {
            let map: serde_json::Map<String, serde_json::Value> = t
                .iter()
                .map(|(k, v)| (k.clone(), toml_to_json(v)))
                .collect();
            serde_json::Value::Object(map)
        }
    }
}

fn json_to_toml(val: &serde_json::Value) -> toml::Value {
    match val {
        serde_json::Value::Null => toml::Value::String("null".into()),
        serde_json::Value::Bool(b) => toml::Value::Boolean(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                toml::Value::Integer(i)
            } else if let Some(f) = n.as_f64() {
                toml::Value::Float(f)
            } else {
                toml::Value::String(n.to_string())
            }
        }
        serde_json::Value::String(s) => toml::Value::String(s.clone()),
        serde_json::Value::Array(a) => toml::Value::Array(a.iter().map(json_to_toml).collect()),
        serde_json::Value::Object(o) => {
            let mut table = toml::map::Map::new();
            for (k, v) in o {
                table.insert(k.clone(), json_to_toml(v));
            }
            toml::Value::Table(table)
        }
    }
}
