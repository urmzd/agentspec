use anyhow::{Context, Result};
use std::path::PathBuf;

/// Get the configuration directory for a tool (`~/.config/<tool_name>/`).
pub fn config_dir(tool_name: &str) -> Result<PathBuf> {
    let base = dirs::config_dir().context("cannot determine config directory")?;
    Ok(base.join(tool_name))
}

/// Get the data directory for a tool (`~/.local/share/<tool_name>/`).
pub fn data_dir(tool_name: &str) -> Result<PathBuf> {
    let base = dirs::data_dir().context("cannot determine data directory")?;
    Ok(base.join(tool_name))
}

/// Get the cache directory for a tool (`~/.cache/<tool_name>/`).
pub fn cache_dir(tool_name: &str) -> Result<PathBuf> {
    let base = dirs::cache_dir().context("cannot determine cache directory")?;
    Ok(base.join(tool_name))
}

/// Ensure a directory exists, creating it if necessary.
pub fn ensure_dir(path: &PathBuf) -> Result<()> {
    if !path.exists() {
        std::fs::create_dir_all(path)
            .with_context(|| format!("failed to create directory: {}", path.display()))?;
    }
    Ok(())
}

/// Load and parse a YAML config file. Returns `None` if the file doesn't exist.
pub fn load_yaml<T: serde::de::DeserializeOwned>(path: &PathBuf) -> Result<Option<T>> {
    if !path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let config: T = serde_yaml_ng::from_str(&content)
        .with_context(|| format!("failed to parse YAML: {}", path.display()))?;
    Ok(Some(config))
}

/// Load and parse a TOML config file. Returns `None` if the file doesn't exist.
pub fn load_toml<T: serde::de::DeserializeOwned>(path: &PathBuf) -> Result<Option<T>> {
    if !path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let config: T = toml::from_str(&content)
        .with_context(|| format!("failed to parse TOML: {}", path.display()))?;
    Ok(Some(config))
}

/// Merge two JSON values. Keys in `overlay` overwrite those in `base`.
pub fn merge_json(base: &serde_json::Value, overlay: &serde_json::Value) -> serde_json::Value {
    match (base, overlay) {
        (serde_json::Value::Object(b), serde_json::Value::Object(o)) => {
            let mut merged = b.clone();
            for (key, val) in o {
                let base_val = merged.get(key).cloned().unwrap_or(serde_json::Value::Null);
                merged.insert(key.clone(), merge_json(&base_val, val));
            }
            serde_json::Value::Object(merged)
        }
        (_, overlay) => overlay.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_dir_includes_tool_name() {
        let path = config_dir("my-tool").unwrap();
        assert!(path.ends_with("my-tool"));
    }

    #[test]
    fn data_dir_includes_tool_name() {
        let path = data_dir("my-tool").unwrap();
        assert!(path.ends_with("my-tool"));
    }

    #[test]
    fn cache_dir_includes_tool_name() {
        let path = cache_dir("my-tool").unwrap();
        assert!(path.ends_with("my-tool"));
    }

    #[test]
    fn load_yaml_returns_none_for_missing_file() {
        let path = PathBuf::from("/tmp/nonexistent-file.yaml");
        let result: Result<Option<serde_json::Value>> = load_yaml(&path);
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn merge_json_deep() {
        let base = serde_json::json!({"a": {"b": 1, "c": 2}, "d": 3});
        let overlay = serde_json::json!({"a": {"b": 10, "e": 5}});
        let merged = merge_json(&base, &overlay);
        assert_eq!(
            merged,
            serde_json::json!({"a": {"b": 10, "c": 2, "e": 5}, "d": 3})
        );
    }

    #[test]
    fn merge_json_overlay_wins_on_type_mismatch() {
        let base = serde_json::json!({"a": [1, 2]});
        let overlay = serde_json::json!({"a": "replaced"});
        let merged = merge_json(&base, &overlay);
        assert_eq!(merged, serde_json::json!({"a": "replaced"}));
    }
}
