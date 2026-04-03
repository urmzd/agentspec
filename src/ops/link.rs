use std::path::Path;

use crate::config;
use crate::error::{AppError, Result};
use crate::ir::ResourceKind;
use crate::tools;

pub fn link(kind: ResourceKind, name: &str, tool_slug: &str) -> Result<()> {
    let tool =
        tools::find_tool(tool_slug).ok_or_else(|| AppError::ToolNotFound(tool_slug.into()))?;

    let (shared_dir, tool_dir) = match kind {
        ResourceKind::Skill => {
            let shared = config::shared_skills_dir().join(name);
            if !shared.exists() {
                return Err(AppError::SkillNotFound(name.into()));
            }
            let dir = tool.skills_dir().ok_or_else(|| {
                AppError::Other(format!("{} doesn't support skills", tool.name()))
            })?;
            (shared, dir)
        }
        ResourceKind::Agent => {
            let shared = config::shared_agents_dir().join(format!("{name}.md"));
            if !shared.exists() {
                return Err(AppError::AgentNotFound(name.into()));
            }
            let dir = tool.agents_dir().ok_or_else(|| {
                AppError::Other(format!("{} doesn't support agents", tool.name()))
            })?;
            (shared, dir)
        }
    };

    std::fs::create_dir_all(&tool_dir)?;

    let link_path = match kind {
        ResourceKind::Skill => tool_dir.join(name),
        ResourceKind::Agent => tool_dir.join(format!("{name}.md")),
    };

    if link_path.exists() || link_path.is_symlink() {
        return Err(AppError::AlreadyExists(format!(
            "{name} is already linked to {}",
            tool.slug()
        )));
    }

    let target = make_relative(&link_path, &shared_dir);
    std::os::unix::fs::symlink(&target, &link_path)?;

    println!("Linked {} '{}' to {}", kind, name, tool.name());
    Ok(())
}

pub fn unlink(kind: ResourceKind, name: &str, tool_slug: &str) -> Result<()> {
    let tool =
        tools::find_tool(tool_slug).ok_or_else(|| AppError::ToolNotFound(tool_slug.into()))?;

    let tool_dir = match kind {
        ResourceKind::Skill => tool.skills_dir(),
        ResourceKind::Agent => tool.agents_dir(),
    }
    .ok_or_else(|| AppError::Other(format!("{} doesn't support {}s", tool.name(), kind)))?;

    let link_path = match kind {
        ResourceKind::Skill => tool_dir.join(name),
        ResourceKind::Agent => tool_dir.join(format!("{name}.md")),
    };

    if !link_path.is_symlink() {
        return Err(AppError::Other(format!(
            "{name} is not linked to {}",
            tool.slug()
        )));
    }

    std::fs::remove_file(&link_path)?;
    println!("Unlinked {} '{}' from {}", kind, name, tool.name());
    Ok(())
}

pub fn link_to_tools(kind: ResourceKind, name: &str, tool_slugs: &[String]) -> Result<()> {
    for slug in tool_slugs {
        if let Err(e) = link(kind, name, slug) {
            eprintln!("Warning: could not link to {slug}: {e}");
        }
    }
    Ok(())
}

pub fn unlink_from_all(kind: ResourceKind, name: &str) -> Result<()> {
    for tool in tools::installed_tools() {
        let linked = match kind {
            ResourceKind::Skill => tool.linked_skills(),
            ResourceKind::Agent => tool.linked_agents(),
        };
        if linked.contains(&name.to_string()) {
            unlink(kind, name, tool.slug())?;
        }
    }
    Ok(())
}

/// Compute a relative path from `from` to `to`.
fn make_relative(from: &Path, to: &Path) -> std::path::PathBuf {
    let from_dir = from.parent().unwrap();
    pathdiff::diff_paths(to, from_dir).unwrap_or_else(|| to.to_path_buf())
}
