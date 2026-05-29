//! Plugin tracking.
//!
//! Claude Code records installed plugins in
//! `~/.claude/plugins/installed_plugins.json` with versions and git commit
//! SHAs. `plugins list` inventories them; `plugins export` writes a portable
//! manifest (`~/.agents/plugins.yml` or a chosen path) so a machine's plugin
//! set can be reproduced.

use std::collections::HashMap;
use std::path::PathBuf;

use console::style;
use serde::{Deserialize, Serialize};

use crate::config;
use crate::error::{AppError, Result};

#[derive(Debug, Deserialize)]
struct InstalledPlugins {
    #[serde(default)]
    plugins: HashMap<String, Vec<PluginInstall>>,
}

#[derive(Debug, Clone, Deserialize)]
struct PluginInstall {
    #[serde(default)]
    scope: String,
    #[serde(default)]
    version: String,
    #[serde(default, rename = "gitCommitSha")]
    git_commit_sha: String,
}

/// One entry in the portable manifest.
#[derive(Debug, Clone, Serialize)]
struct ManifestEntry {
    plugin: String,
    marketplace: String,
    scope: String,
    version: String,
    git_commit_sha: String,
}

fn installed_plugins_file() -> PathBuf {
    config::home_dir()
        .join(".claude")
        .join("plugins")
        .join("installed_plugins.json")
}

fn load() -> Result<InstalledPlugins> {
    let path = installed_plugins_file();
    if !path.exists() {
        return Ok(InstalledPlugins {
            plugins: HashMap::new(),
        });
    }
    let text = std::fs::read_to_string(&path)?;
    serde_json::from_str(&text)
        .map_err(|e| AppError::Other(format!("invalid installed_plugins.json: {e}")))
}

/// Split a `plugin@marketplace` key into its parts.
fn split_key(key: &str) -> (String, String) {
    match key.rsplit_once('@') {
        Some((p, m)) => (p.to_string(), m.to_string()),
        None => (key.to_string(), String::new()),
    }
}

fn entries(installed: &InstalledPlugins) -> Vec<ManifestEntry> {
    let mut out = Vec::new();
    for (key, installs) in &installed.plugins {
        let (plugin, marketplace) = split_key(key);
        for inst in installs {
            out.push(ManifestEntry {
                plugin: plugin.clone(),
                marketplace: marketplace.clone(),
                scope: inst.scope.clone(),
                version: inst.version.clone(),
                git_commit_sha: inst.git_commit_sha.clone(),
            });
        }
    }
    out.sort_by(|a, b| (&a.plugin, &a.marketplace).cmp(&(&b.plugin, &b.marketplace)));
    out
}

/// List installed plugins.
pub fn list_plugins(json: bool) -> Result<()> {
    let installed = load()?;
    let entries = entries(&installed);

    if json {
        println!("{}", serde_json::to_string_pretty(&entries)?);
        return Ok(());
    }
    if entries.is_empty() {
        println!(
            "  no plugins found in {}",
            installed_plugins_file().display()
        );
        return Ok(());
    }
    for e in &entries {
        let sha = if e.git_commit_sha.len() >= 8 {
            &e.git_commit_sha[..8]
        } else {
            e.git_commit_sha.as_str()
        };
        println!(
            "  {} {}@{:<28} {:<10} {:<8} {}",
            style("●").green(),
            e.plugin,
            e.marketplace,
            e.version,
            sha,
            e.scope
        );
    }
    Ok(())
}

/// Export a portable plugin manifest.
pub fn export_plugins(output: Option<&str>, json: bool) -> Result<()> {
    let installed = load()?;
    let entries = entries(&installed);

    let serialized = if json {
        serde_json::to_string_pretty(&entries)?
    } else {
        serde_yaml::to_string(&entries)
            .map_err(|e| AppError::Other(format!("serialize failed: {e}")))?
    };

    match output {
        Some(path) => {
            std::fs::write(path, &serialized)?;
            eprintln!("  wrote {} plugin(s) to {path}", entries.len());
        }
        None => {
            // Default to the canonical manifest path for `export` with no -o,
            // and also echo to stdout for piping.
            let manifest = config::plugins_manifest();
            if let Some(parent) = manifest.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let yaml = serde_yaml::to_string(&entries)
                .map_err(|e| AppError::Other(format!("serialize failed: {e}")))?;
            std::fs::write(&manifest, &yaml)?;
            eprintln!(
                "  wrote {} plugin(s) to {}",
                entries.len(),
                manifest.display()
            );
            print!("{serialized}");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_plugin_marketplace_key() {
        assert_eq!(
            split_key("gopls-lsp@claude-plugins-official"),
            (
                "gopls-lsp".to_string(),
                "claude-plugins-official".to_string()
            )
        );
        assert_eq!(split_key("bare"), ("bare".to_string(), String::new()));
    }

    #[test]
    fn parses_and_flattens_installed_plugins() {
        let json = r#"{
            "version": 2,
            "plugins": {
                "a@m1": [{"scope":"user","version":"1.0.0","gitCommitSha":"abcdef0123"}],
                "b@m2": [{"scope":"project","version":"unknown","gitCommitSha":""}]
            }
        }"#;
        let parsed: InstalledPlugins = serde_json::from_str(json).unwrap();
        let e = entries(&parsed);
        assert_eq!(e.len(), 2);
        // Sorted by plugin name.
        assert_eq!(e[0].plugin, "a");
        assert_eq!(e[0].marketplace, "m1");
        assert_eq!(e[0].git_commit_sha, "abcdef0123");
        assert_eq!(e[1].plugin, "b");
        assert_eq!(e[1].version, "unknown");
    }
}
