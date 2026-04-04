use std::path::Path;

use console::style;

use crate::config;
use crate::error::{AppError, Result};
use crate::inventory::{self, Config, SourceType, TrackedKind, TrackedResource, hash_resource};
use crate::ir::ResourceKind;
use crate::ops::link;
use crate::tools;

#[derive(Debug)]
enum SourceKind {
    Local(String),
    Git(String),
}

fn resolve_source(input: &str) -> SourceKind {
    // Local path
    if Path::new(input).exists() {
        return SourceKind::Local(input.to_string());
    }

    // Explicit git URL
    if input.starts_with("https://")
        || input.starts_with("git://")
        || input.starts_with("ssh://")
        || input.ends_with(".git")
    {
        return SourceKind::Git(input.to_string());
    }

    // GitHub shorthand: owner/repo
    if input.contains('/') {
        return SourceKind::Git(format!("https://github.com/{input}.git"));
    }

    // Fall back to local path (will fail later if it doesn't exist)
    SourceKind::Local(input.to_string())
}

pub fn manage(
    source: &str,
    tool_slugs: Option<&[String]>,
    all_tools: bool,
    copy: bool,
) -> Result<()> {
    match resolve_source(source) {
        SourceKind::Local(path) => manage_local(&path, source, tool_slugs, all_tools, copy),
        SourceKind::Git(url) => manage_git(&url, source, tool_slugs, all_tools, copy),
    }
}

fn manage_local(
    path: &str,
    source: &str,
    tool_slugs: Option<&[String]>,
    all_tools: bool,
    copy: bool,
) -> Result<()> {
    let p = Path::new(path);
    if !p.exists() {
        return Err(AppError::Other(format!("path not found: {path}")));
    }

    let mut lockfile = inventory::load_config()?;
    let installed = install_from_dir(
        p,
        source,
        &mut lockfile,
        tool_slugs,
        all_tools,
        copy,
        SourceType::Local,
    )?;

    inventory::save_config(&lockfile)?;

    println!("\n  {} resource(s) managed from {source}", installed);

    Ok(())
}

fn manage_git(
    url: &str,
    source: &str,
    tool_slugs: Option<&[String]>,
    all_tools: bool,
    copy: bool,
) -> Result<()> {
    let tmp = tempfile::tempdir().map_err(AppError::Io)?;

    println!("  {} Cloning {source}...", style("↓").cyan().bold());

    let status = std::process::Command::new("git")
        .args(["clone", "--depth", "1", url, tmp.path().to_str().unwrap()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|e| AppError::Git(format!("failed to run git: {e}")))?;

    if !status.success() {
        return Err(AppError::Git(format!("git clone failed for {url}")));
    }

    let mut lockfile = inventory::load_config()?;
    let installed = install_from_dir(
        tmp.path(),
        source,
        &mut lockfile,
        tool_slugs,
        all_tools,
        copy,
        SourceType::Git,
    )?;

    inventory::save_config(&lockfile)?;

    println!("\n  {} resource(s) managed from {source}", installed);

    Ok(())
}

/// Install skills and agents from a directory, updating the lockfile.
fn install_from_dir(
    dir: &Path,
    source: &str,
    lockfile: &mut Config,
    tool_slugs: Option<&[String]>,
    all_tools: bool,
    copy: bool,
    source_type: SourceType,
) -> Result<usize> {
    let mut installed = 0;

    // Find and install skills (directories with SKILL.md)
    for entry in walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_name() != "SKILL.md" {
            continue;
        }
        let skill_dir = entry.path().parent().unwrap();
        let name = skill_dir.file_name().unwrap().to_string_lossy().to_string();
        let dest = config::shared_skills_dir().join(&name);

        if dest.exists() {
            println!("  {} {name} already exists, skipping", style("~").yellow());
            continue;
        }

        copy_dir(skill_dir, &dest)?;

        let hash = hash_resource(TrackedKind::Skill, &dest)?;
        let source_str = match source_type {
            SourceType::Git => source.to_string(),
            SourceType::Local => source.to_string(),
            SourceType::Discovered => "discovered".to_string(),
        };

        let tracked = TrackedResource::new(
            name.clone(),
            TrackedKind::Skill,
            source_str,
            source_type,
            format!("skills/{name}"),
            hash,
        );
        lockfile.add(tracked);

        let slugs = resolve_tool_slugs(tool_slugs, all_tools);
        if !slugs.is_empty() {
            link::link_to_tools(ResourceKind::Skill, &name, &slugs, copy)?;
        }

        println!("  {} Managed skill '{}'", style("✓").green().bold(), name);
        installed += 1;
    }

    // Find and install agents (.md files with frontmatter)
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
        let dest = config::shared_agents_dir().join(format!("{name}.md"));

        if dest.exists() {
            println!("  {} {name} already exists, skipping", style("~").yellow());
            continue;
        }

        std::fs::copy(path, &dest)?;

        let hash = hash_resource(TrackedKind::Agent, &dest)?;
        let source_str = match source_type {
            SourceType::Git => source.to_string(),
            SourceType::Local => source.to_string(),
            SourceType::Discovered => "discovered".to_string(),
        };

        let tracked = TrackedResource::new(
            name.clone(),
            TrackedKind::Agent,
            source_str,
            source_type,
            format!("agents/{name}.md"),
            hash,
        );
        lockfile.add(tracked);

        let slugs = resolve_tool_slugs(tool_slugs, all_tools);
        if !slugs.is_empty() {
            link::link_to_tools(ResourceKind::Agent, &name, &slugs, copy)?;
        }

        println!("  {} Managed agent '{}'", style("✓").green().bold(), name);
        installed += 1;
    }

    Ok(installed)
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
