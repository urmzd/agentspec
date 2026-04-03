use std::path::Path;

use console::style;

use crate::config;
use crate::error::{AppError, Result};
use crate::ir::ResourceKind;
use crate::lockfile::{self, LockFile, LockedEntry};
use crate::ops::link;
use crate::tools;

pub fn install_skill(source: &str, tool_slugs: Option<&[String]>, all_tools: bool) -> Result<()> {
    let is_github = source.contains('/') && !Path::new(source).exists();

    if is_github {
        install_from_github(source, ResourceKind::Skill, tool_slugs, all_tools)
    } else {
        install_from_local(source, ResourceKind::Skill, tool_slugs, all_tools)
    }
}

pub fn install_agent(source: &str, tool_slugs: Option<&[String]>, all_tools: bool) -> Result<()> {
    let is_github = source.contains('/') && !Path::new(source).exists();

    if is_github {
        install_from_github(source, ResourceKind::Agent, tool_slugs, all_tools)
    } else {
        install_from_local(source, ResourceKind::Agent, tool_slugs, all_tools)
    }
}

fn install_from_github(
    source: &str,
    kind: ResourceKind,
    tool_slugs: Option<&[String]>,
    all_tools: bool,
) -> Result<()> {
    let url = format!("https://github.com/{source}.git");
    let tmp = tempfile::tempdir().map_err(AppError::Io)?;

    println!("  {} Cloning {source}...", style("↓").cyan().bold());

    let status = std::process::Command::new("git")
        .args(["clone", "--depth", "1", &url, tmp.path().to_str().unwrap()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|e| AppError::Git(format!("failed to run git: {e}")))?;

    if !status.success() {
        return Err(AppError::Git(format!("git clone failed for {source}")));
    }

    match kind {
        ResourceKind::Skill => install_skills_from_dir(tmp.path(), source, tool_slugs, all_tools),
        ResourceKind::Agent => install_agents_from_dir(tmp.path(), source, tool_slugs, all_tools),
    }
}

fn install_from_local(
    source: &str,
    kind: ResourceKind,
    tool_slugs: Option<&[String]>,
    all_tools: bool,
) -> Result<()> {
    let path = Path::new(source);
    if !path.exists() {
        return Err(AppError::Other(format!("path not found: {source}")));
    }

    match kind {
        ResourceKind::Skill => install_skills_from_dir(path, source, tool_slugs, all_tools),
        ResourceKind::Agent => install_agents_from_dir(path, source, tool_slugs, all_tools),
    }
}

fn install_skills_from_dir(
    dir: &Path,
    source: &str,
    tool_slugs: Option<&[String]>,
    all_tools: bool,
) -> Result<()> {
    let mut lock = LockFile::load(&config::lock_file_path())?;
    let dest_base = config::shared_skills_dir();
    let mut installed = 0;

    // Find all SKILL.md files
    for entry in walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_name() != "SKILL.md" {
            continue;
        }
        let skill_dir = entry.path().parent().unwrap();
        let name = skill_dir.file_name().unwrap().to_string_lossy().to_string();
        let dest = dest_base.join(&name);

        if dest.exists() {
            println!(
                "  {} {name} already installed, skipping",
                style("~").yellow()
            );
            continue;
        }

        copy_dir(skill_dir, &dest)?;

        let hash = lockfile::compute_folder_hash(&dest)?;
        let relative_skill_path = entry
            .path()
            .strip_prefix(dir)
            .unwrap_or(entry.path())
            .to_string_lossy()
            .to_string();

        let entry = LockedEntry::new_github(source, &relative_skill_path, &hash);
        lock.add_entry(name.clone(), entry);

        let slugs = resolve_tool_slugs(tool_slugs, all_tools);
        if !slugs.is_empty() {
            link::link_to_tools(ResourceKind::Skill, &name, &slugs)?;
        }

        println!("  {} Installed skill '{}'", style("✓").green().bold(), name);
        installed += 1;
    }

    lock.save(&config::lock_file_path())?;
    println!("\n  {} skill(s) installed from {source}", installed);
    Ok(())
}

fn install_agents_from_dir(
    dir: &Path,
    source: &str,
    tool_slugs: Option<&[String]>,
    all_tools: bool,
) -> Result<()> {
    let dest_base = config::shared_agents_dir();
    let mut installed = 0;

    // Find .md files with YAML frontmatter containing "name" and "description"
    for entry in walkdir::WalkDir::new(dir)
        .max_depth(3)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        if path.file_name().unwrap().to_str() == Some("SKILL.md") {
            continue;
        }

        // Quick check: does it have frontmatter with name + description?
        let content = std::fs::read_to_string(path).unwrap_or_default();
        if !content.starts_with("---")
            || !content.contains("name:")
            || !content.contains("description:")
        {
            continue;
        }

        let name = path.file_stem().unwrap().to_string_lossy().to_string();
        let dest = dest_base.join(format!("{name}.md"));

        if dest.exists() {
            println!(
                "  {} {name} already installed, skipping",
                style("~").yellow()
            );
            continue;
        }

        std::fs::copy(path, &dest)?;

        let slugs = resolve_tool_slugs(tool_slugs, all_tools);
        if !slugs.is_empty() {
            link::link_to_tools(ResourceKind::Agent, &name, &slugs)?;
        }

        println!("  {} Installed agent '{}'", style("✓").green().bold(), name);
        installed += 1;
    }

    println!("\n  {} agent(s) installed from {source}", installed);
    Ok(())
}

fn resolve_tool_slugs(explicit: Option<&[String]>, all: bool) -> Vec<String> {
    if all {
        tools::installed_tools()
            .iter()
            .map(|t| t.slug().to_string())
            .collect()
    } else {
        explicit.map(|s| s.to_vec()).unwrap_or_default()
    }
}

fn copy_dir(src: &Path, dst: &Path) -> Result<()> {
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
