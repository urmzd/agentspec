use std::path::Path;

use crate::config;
use crate::error::{AppError, Result};
use crate::ir::ResourceKind;
use crate::tools;

pub fn link(kind: ResourceKind, name: &str, tool_slug: &str, copy: bool) -> Result<()> {
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
        _ => {
            return Err(AppError::Other(format!(
                "linking {kind} resources is not yet supported"
            )));
        }
    };

    std::fs::create_dir_all(&tool_dir)?;

    let link_path = match kind {
        ResourceKind::Skill => tool_dir.join(name),
        ResourceKind::Agent => tool_dir.join(format!("{name}.md")),
        _ => unreachable!("handled above"),
    };

    if link_path.exists() || link_path.is_symlink() {
        return Err(AppError::AlreadyExists(format!(
            "{name} is already linked to {}",
            tool.slug()
        )));
    }

    if copy {
        if shared_dir.is_dir() {
            copy_dir_recursive(&shared_dir, &link_path)?;
        } else {
            std::fs::copy(&shared_dir, &link_path)?;
        }
        let strategy = "copied";
        println!("  {} {} '{}' to {}", strategy, kind, name, tool.name());
    } else {
        let target = make_relative(&link_path, &shared_dir);
        std::os::unix::fs::symlink(&target, &link_path)?;
        println!("  Linked {} '{}' to {}", kind, name, tool.name());
    }

    Ok(())
}

pub fn unlink(kind: ResourceKind, name: &str, tool_slug: &str) -> Result<()> {
    let tool =
        tools::find_tool(tool_slug).ok_or_else(|| AppError::ToolNotFound(tool_slug.into()))?;

    let tool_dir = match kind {
        ResourceKind::Skill => tool.skills_dir(),
        ResourceKind::Agent => tool.agents_dir(),
        _ => {
            return Err(AppError::Other(format!(
                "unlinking {kind} resources is not yet supported"
            )));
        }
    }
    .ok_or_else(|| AppError::Other(format!("{} doesn't support {}s", tool.name(), kind)))?;

    let link_path = match kind {
        ResourceKind::Skill => tool_dir.join(name),
        ResourceKind::Agent => tool_dir.join(format!("{name}.md")),
        _ => unreachable!("handled above"),
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

pub fn link_to_tools(
    kind: ResourceKind,
    name: &str,
    tool_slugs: &[String],
    copy: bool,
) -> Result<()> {
    for slug in tool_slugs {
        if let Err(e) = link(kind, name, slug, copy) {
            eprintln!("Warning: could not link to {slug}: {e}");
        }
    }
    Ok(())
}

pub fn unlink_from_all(kind: ResourceKind, name: &str) -> Result<()> {
    if !matches!(kind, ResourceKind::Skill | ResourceKind::Agent) {
        return Ok(()); // only skill/agent have tool symlinks
    }
    for tool in tools::installed_tools() {
        let linked = match kind {
            ResourceKind::Skill => tool.linked_skills(),
            ResourceKind::Agent => tool.linked_agents(),
            _ => continue,
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

/// Public version of make_relative for use by other modules.
pub fn make_relative_public(from: &Path, to: &Path) -> std::path::PathBuf {
    make_relative(from, to)
}

/// Ensure all managed resources are symlinked to all installed tools.
/// Creates missing symlinks, skips existing ones. Returns count of newly created links.
pub fn ensure_all_links(copy: bool) -> Result<usize> {
    use crate::inventory;

    let cfg = inventory::load_config()?;
    let installed = tools::installed_tools();
    let mut created = 0;

    for resource in &cfg.resources {
        let kind: ResourceKind = resource.kind.into();
        if !matches!(kind, ResourceKind::Skill | ResourceKind::Agent) {
            continue;
        }

        for tool in &installed {
            let tool_dir = match kind {
                ResourceKind::Skill => tool.skills_dir(),
                ResourceKind::Agent => tool.agents_dir(),
                _ => None,
            };
            let Some(tool_dir) = tool_dir else { continue };

            let link_path = match kind {
                ResourceKind::Skill => tool_dir.join(&resource.name),
                ResourceKind::Agent => tool_dir.join(format!("{}.md", resource.name)),
                _ => continue,
            };

            // Already exists (symlink or file) — skip
            if link_path.exists() || link_path.is_symlink() {
                continue;
            }

            // Create parent dir if needed
            std::fs::create_dir_all(&tool_dir)?;

            let shared_path = match kind {
                ResourceKind::Skill => config::shared_skills_dir().join(&resource.name),
                ResourceKind::Agent => {
                    config::shared_agents_dir().join(format!("{}.md", resource.name))
                }
                _ => continue,
            };

            if !shared_path.exists() {
                continue;
            }

            if copy {
                if shared_path.is_dir() {
                    copy_dir_recursive(&shared_path, &link_path)?;
                } else {
                    std::fs::copy(&shared_path, &link_path)?;
                }
            } else {
                let target = make_relative(&link_path, &shared_path);
                std::os::unix::fs::symlink(&target, &link_path)?;
            }
            created += 1;
        }
    }

    Ok(created)
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in walkdir::WalkDir::new(src)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let relative = entry.path().strip_prefix(src).unwrap();
        let target = dst.join(relative);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&target)?;
        } else {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}
