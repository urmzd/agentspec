//! Small helpers for read-modify-write of JSON config files (tool settings,
//! `.mcp.json`, etc.) with dotted-key access. Shared by the MCP and permission
//! subsystems so there is one JSON-editing code path.

use std::path::Path;

use serde_json::Value;

use crate::error::Result;

/// Read a JSON file, returning an empty object if missing or invalid.
pub(crate) fn read_json(path: &Path) -> Value {
    if !path.exists() {
        return serde_json::json!({});
    }
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| serde_json::json!({}))
}

/// Write a JSON value to a file (pretty), creating parent dirs as needed.
pub(crate) fn write_json(path: &Path, val: &Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(val)?;
    std::fs::write(path, content)?;
    Ok(())
}

/// Read a dotted key path (e.g. `permissions.allow`) from a JSON value.
pub(crate) fn get_dotted(val: &Value, key: &str) -> Option<Value> {
    let mut current = val;
    for part in key.split('.') {
        current = current.get(part)?;
    }
    Some(current.clone())
}

/// Set a dotted key path, creating intermediate objects as needed. Any
/// intermediate value that is not an object is replaced with one.
pub(crate) fn set_dotted(val: &mut Value, key: &str, new_val: Value) {
    let parts: Vec<&str> = key.split('.').collect();
    let mut current = val;
    for part in &parts[..parts.len() - 1] {
        if !current.is_object() {
            *current = serde_json::json!({});
        }
        current = current
            .as_object_mut()
            .unwrap()
            .entry((*part).to_string())
            .or_insert_with(|| serde_json::json!({}));
    }
    if !current.is_object() {
        *current = serde_json::json!({});
    }
    current
        .as_object_mut()
        .unwrap()
        .insert(parts.last().unwrap().to_string(), new_val);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dotted_roundtrip() {
        let mut v = serde_json::json!({});
        set_dotted(&mut v, "permissions.allow", serde_json::json!(["Bash(ls)"]));
        assert_eq!(
            get_dotted(&v, "permissions.allow").unwrap(),
            serde_json::json!(["Bash(ls)"])
        );
        // Preserves siblings.
        set_dotted(&mut v, "permissions.deny", serde_json::json!([]));
        assert_eq!(
            get_dotted(&v, "permissions.allow").unwrap(),
            serde_json::json!(["Bash(ls)"])
        );
    }

    #[test]
    fn replaces_non_object_intermediate() {
        let mut v = serde_json::json!({ "permissions": "oops" });
        set_dotted(&mut v, "permissions.allow", serde_json::json!(["x"]));
        assert_eq!(
            get_dotted(&v, "permissions.allow").unwrap(),
            serde_json::json!(["x"])
        );
    }
}
