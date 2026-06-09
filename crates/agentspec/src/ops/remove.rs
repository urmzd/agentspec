use console::style;

use crate::config;
use crate::error::{AppError, Result};
use crate::inventory::{Config, TrackedKind};
use crate::ir::ResourceKind;
use crate::ops::link;

pub fn remove_skill(cfg: &mut Config, name: &str) -> Result<()> {
    let skill_dir = config::shared_skills_dir().join(name);
    if !skill_dir.exists() {
        return Err(AppError::SkillNotFound(name.into()));
    }

    // Unlink from all tools first
    link::unlink_from_all(cfg, ResourceKind::Skill, name)?;

    // Remove from inventory config
    cfg.remove(name, TrackedKind::Skill);

    // Remove directory
    std::fs::remove_dir_all(&skill_dir)?;

    eprintln!("  {} Removed skill '{name}'", style("✓").green().bold());
    Ok(())
}

pub fn remove_agent(cfg: &mut Config, name: &str) -> Result<()> {
    let agent_file = config::shared_agents_dir().join(format!("{name}.md"));
    if !agent_file.exists() {
        return Err(AppError::AgentNotFound(name.into()));
    }

    link::unlink_from_all(cfg, ResourceKind::Agent, name)?;

    // Remove from inventory config
    cfg.remove(name, TrackedKind::Agent);

    std::fs::remove_file(&agent_file)?;

    eprintln!("  {} Removed agent '{name}'", style("✓").green().bold());
    Ok(())
}

pub fn remove_unmanaged_agent(name: &str, paths: &[std::path::PathBuf]) -> Result<()> {
    for path in paths {
        if path.exists() {
            std::fs::remove_file(path)?;
        }
    }

    eprintln!(
        "  {} Removed unmanaged agent '{name}'",
        style("✓").green().bold()
    );
    Ok(())
}

/// Remove tracking for a resource without deleting the underlying file.
/// Used for memories, project configs, llms.txt — files owned by their
/// respective tools, not by agentspec.
pub fn remove_tracked(cfg: &mut Config, name: &str, kind: TrackedKind) -> Result<()> {
    if cfg.find(name, kind).is_none() {
        return Err(AppError::Other(format!("{kind} '{name}' is not tracked")));
    }

    cfg.remove(name, kind);

    eprintln!(
        "  {} Removed {kind} '{name}' from tracking",
        style("✓").green().bold()
    );
    Ok(())
}
