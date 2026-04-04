use std::path::Path;

use console::style;

use crate::config;
use crate::error::{AppError, Result};
use crate::inventory::{self, SourceType, TrackedKind, TrackedResource, hash_resource};
use crate::ir::ResourceKind;
use crate::lockfile::{self, LockFile, LockedEntry};
use crate::ops::link;
use crate::tools;

pub fn install_skill(source: &str, tool_slugs: Option<&[String]>, all_tools: bool) -> Result<()> {
    let is_remote = is_remote_source(source);

    if is_remote {
        let url = resolve_git_url(source);
        install_from_git(&url, source, ResourceKind::Skill, tool_slugs, all_tools)
    } else {
        install_from_local(source, ResourceKind::Skill, tool_slugs, all_tools)
    }
}

pub fn install_agent(source: &str, tool_slugs: Option<&[String]>, all_tools: bool) -> Result<()> {
    let is_remote = is_remote_source(source);

    if is_remote {
        let url = resolve_git_url(source);
        install_from_git(&url, source, ResourceKind::Agent, tool_slugs, all_tools)
    } else {
        install_from_local(source, ResourceKind::Agent, tool_slugs, all_tools)
    }
}

/// Check if a source string looks like a remote (git URL or owner/repo shorthand).
fn is_remote_source(source: &str) -> bool {
    if Path::new(source).exists() {
        return false;
    }
    source.starts_with("https://")
        || source.starts_with("git://")
        || source.starts_with("ssh://")
        || source.ends_with(".git")
        || source.contains('/')
}

/// Resolve a source string to a full git URL.
fn resolve_git_url(source: &str) -> String {
    if source.starts_with("https://")
        || source.starts_with("git://")
        || source.starts_with("ssh://")
        || source.ends_with(".git")
    {
        source.to_string()
    } else {
        // GitHub shorthand: owner/repo
        format!("https://github.com/{source}.git")
    }
}

/// Clone a git repo to a temp directory.
pub fn clone_repo(url: &str, display_name: &str) -> Result<tempfile::TempDir> {
    let tmp = tempfile::tempdir().map_err(AppError::Io)?;

    println!("  {} Cloning {display_name}...", style("↓").cyan().bold());

    let status = std::process::Command::new("git")
        .args(["clone", "--depth", "1", url, tmp.path().to_str().unwrap()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|e| AppError::Git(format!("failed to run git: {e}")))?;

    if !status.success() {
        return Err(AppError::Git(format!("git clone failed for {url}")));
    }

    Ok(tmp)
}

fn install_from_git(
    url: &str,
    source: &str,
    kind: ResourceKind,
    tool_slugs: Option<&[String]>,
    all_tools: bool,
) -> Result<()> {
    let tmp = clone_repo(url, source)?;

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
    let mut inv_lock = inventory::load_config()?;
    let dest_base = config::shared_skills_dir();
    let mut installed = 0;

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

        // Legacy lockfile entry
        let legacy_hash = lockfile::compute_folder_hash(&dest)?;
        let relative_skill_path = entry
            .path()
            .strip_prefix(dir)
            .unwrap_or(entry.path())
            .to_string_lossy()
            .to_string();
        let legacy_entry = LockedEntry::new_github(source, &relative_skill_path, &legacy_hash);
        lock.add_entry(name.clone(), legacy_entry);

        // New inventory lockfile entry
        let hash = hash_resource(TrackedKind::Skill, &dest)?;
        let source_type = if is_remote_source(source) {
            SourceType::Git
        } else {
            SourceType::Local
        };
        let tracked = TrackedResource::new(
            name.clone(),
            TrackedKind::Skill,
            source.to_string(),
            source_type,
            format!("skills/{name}"),
            hash,
        );
        inv_lock.add(tracked);

        let slugs = resolve_tool_slugs(tool_slugs, all_tools);
        if !slugs.is_empty() {
            link::link_to_tools(ResourceKind::Skill, &name, &slugs, false)?;
        }

        println!("  {} Installed skill '{}'", style("✓").green().bold(), name);
        installed += 1;
    }

    lock.save(&config::lock_file_path())?;
    inventory::save_config(&inv_lock)?;
    println!("\n  {} skill(s) installed from {source}", installed);
    Ok(())
}

fn install_agents_from_dir(
    dir: &Path,
    source: &str,
    tool_slugs: Option<&[String]>,
    all_tools: bool,
) -> Result<()> {
    let mut inv_lock = inventory::load_config()?;
    let dest_base = config::shared_agents_dir();
    let mut installed = 0;

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

        // New inventory lockfile entry
        let hash = hash_resource(TrackedKind::Agent, &dest)?;
        let source_type = if is_remote_source(source) {
            SourceType::Git
        } else {
            SourceType::Local
        };
        let tracked = TrackedResource::new(
            name.clone(),
            TrackedKind::Agent,
            source.to_string(),
            source_type,
            format!("agents/{name}.md"),
            hash,
        );
        inv_lock.add(tracked);

        let slugs = resolve_tool_slugs(tool_slugs, all_tools);
        if !slugs.is_empty() {
            link::link_to_tools(ResourceKind::Agent, &name, &slugs, false)?;
        }

        println!("  {} Installed agent '{}'", style("✓").green().bold(), name);
        installed += 1;
    }

    inventory::save_config(&inv_lock)?;
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
