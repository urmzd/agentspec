use console::style;

use crate::config;
use crate::error::{AppError, Result};
use crate::inventory::{self, TrackedKind};
use crate::ir::ResourceKind;
use crate::lockfile::LockFile;
use crate::ops::link;

pub fn remove_skill(name: &str) -> Result<()> {
    let skill_dir = config::shared_skills_dir().join(name);
    if !skill_dir.exists() {
        return Err(AppError::SkillNotFound(name.into()));
    }

    // Unlink from all tools first
    link::unlink_from_all(ResourceKind::Skill, name)?;

    // Remove from lock file
    let mut lock = LockFile::load(&config::lock_file_path())?;
    lock.remove_entry(name);
    lock.save(&config::lock_file_path())?;

    // Remove directory
    std::fs::remove_dir_all(&skill_dir)?;

    println!("  {} Removed skill '{name}'", style("✓").green().bold());
    Ok(())
}

pub fn remove_agent(name: &str) -> Result<()> {
    let agent_file = config::shared_agents_dir().join(format!("{name}.md"));
    if !agent_file.exists() {
        return Err(AppError::AgentNotFound(name.into()));
    }

    link::unlink_from_all(ResourceKind::Agent, name)?;
    std::fs::remove_file(&agent_file)?;

    println!("  {} Removed agent '{name}'", style("✓").green().bold());
    Ok(())
}

/// Remove tracking for a resource without deleting the underlying file.
/// Used for memories, project configs, llms.txt — files owned by their
/// respective tools, not by agentspec.
pub fn remove_tracked(name: &str, kind: TrackedKind) -> Result<()> {
    let mut cfg = inventory::load_config()?;

    if cfg.find(name, kind).is_none() {
        return Err(AppError::Other(format!(
            "{kind} '{name}' is not tracked"
        )));
    }

    cfg.remove(name, kind);
    inventory::save_config(&cfg)?;

    println!(
        "  {} Removed {kind} '{name}' from tracking",
        style("✓").green().bold()
    );
    Ok(())
}
