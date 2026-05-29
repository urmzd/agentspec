//! Hook management.
//!
//! Lifecycle hook scripts are stored canonically in `~/.agents/hooks/` and
//! symlinked into each tool's native hooks directory. Today Claude Code is the
//! only tool with a hooks directory (`~/.claude/hooks/`); the design is
//! extensible — add a tool's hooks dir to [`tool_hooks_dir`] as others gain
//! hook support.

use std::path::{Path, PathBuf};

use console::style;
use serde::Serialize;

use crate::config;
use crate::error::{AppError, Result};
use crate::ops::link;
use crate::tools;

/// The native hooks directory for a tool, if it has one.
fn tool_hooks_dir(slug: &str) -> Option<PathBuf> {
    match slug {
        "claude-code" => Some(config::home_dir().join(".claude").join("hooks")),
        _ => None,
    }
}

/// Installed tools that support hooks, as (slug, hooks dir).
fn hooks_capable_installed() -> Vec<(String, PathBuf)> {
    tools::installed_tools()
        .into_iter()
        .filter_map(|t| tool_hooks_dir(t.slug()).map(|d| (t.slug().to_string(), d)))
        .collect()
}

fn read_names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().to_string())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

/// Add a hook script into the canonical store.
pub fn add_hook(path_str: &str) -> Result<()> {
    let src = Path::new(path_str);
    if !src.is_file() {
        return Err(AppError::Other(format!("hook file not found: {path_str}")));
    }
    let store = config::shared_hooks_dir();
    std::fs::create_dir_all(&store)?;
    let fname = src
        .file_name()
        .ok_or_else(|| AppError::Other("invalid hook path".into()))?;
    let dest = store.join(fname);
    std::fs::copy(src, &dest)?;
    eprintln!(
        "  {} added hook '{}'",
        style("✓").green().bold(),
        fname.to_string_lossy()
    );
    Ok(())
}

/// Symlink a stored hook into tool hooks directories.
pub fn link_hook(name: &str, tool: Option<&str>, all_tools: bool) -> Result<()> {
    let store = config::shared_hooks_dir();
    let src = store.join(name);
    if !src.exists() {
        return Err(AppError::Other(format!(
            "hook '{name}' not found in store ({})",
            store.display()
        )));
    }

    let targets: Vec<(String, PathBuf)> = if let Some(t) = tool.filter(|_| !all_tools) {
        match tool_hooks_dir(t) {
            Some(d) if tools::find_tool(t).is_some() => vec![(t.to_string(), d)],
            Some(_) => return Err(AppError::Other(format!("tool '{t}' not installed"))),
            None => return Err(AppError::Other(format!("tool '{t}' has no hooks support"))),
        }
    } else {
        hooks_capable_installed()
    };

    if targets.is_empty() {
        return Err(AppError::Other(
            "no hooks-capable tools installed (currently: claude-code)".into(),
        ));
    }

    for (slug, dir) in &targets {
        std::fs::create_dir_all(dir)?;
        let link_path = dir.join(name);
        if link_path.exists() || link_path.is_symlink() {
            eprintln!("  {} {name} already linked to {slug}", style("~").yellow());
            continue;
        }
        let target = link::make_relative_public(&link_path, &src);
        std::os::unix::fs::symlink(&target, &link_path)?;
        eprintln!("  {} linked {name} → {slug}", style("✓").green().bold());
    }
    Ok(())
}

#[derive(Serialize)]
struct HookListing {
    canonical: Vec<String>,
    tools: Vec<ToolHooks>,
}

#[derive(Serialize)]
struct ToolHooks {
    tool: String,
    hooks: Vec<String>,
}

/// List hooks in the canonical store and each tool's hooks directory.
pub fn list_hooks(json: bool) -> Result<()> {
    let canonical = read_names(&config::shared_hooks_dir());
    let tools: Vec<ToolHooks> = hooks_capable_installed()
        .into_iter()
        .map(|(slug, dir)| ToolHooks {
            tool: slug,
            hooks: read_names(&dir),
        })
        .collect();

    if json {
        let listing = HookListing { canonical, tools };
        println!("{}", serde_json::to_string_pretty(&listing)?);
        return Ok(());
    }

    println!("{}", style("canonical store (~/.agents/hooks):").bold());
    if canonical.is_empty() {
        println!("  (none)");
    } else {
        for h in &canonical {
            println!("  {} {h}", style("●").green());
        }
    }
    for t in &tools {
        println!("{}:", t.tool);
        if t.hooks.is_empty() {
            println!("  (none)");
        } else {
            for h in &t.hooks {
                println!("  {h}");
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_claude_has_hooks_dir() {
        assert!(tool_hooks_dir("claude-code").is_some());
        assert!(tool_hooks_dir("gemini-cli").is_none());
        assert!(tool_hooks_dir("cursor").is_none());
    }
}
