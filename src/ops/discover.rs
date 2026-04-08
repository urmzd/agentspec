use std::path::{Path, PathBuf};

use console::style;

use crate::config;
use crate::error::{AppError, Result};
use crate::frontmatter;
use crate::project_files;
use crate::inventory::{
    self, Config, DiscoveredResource, DiscoveryLocation, LinkStrategy, ResourceLink, SourceType,
    TrackedKind, TrackedResource, hash_resource,
};
use crate::ir::ResourceKind;
use crate::ops::link;
use crate::ops::memory;
use crate::tools::{self, CodingTool};

/// Refresh the discovery cache. Pass `broad_root` for a broad scan from a root directory.
/// Pass `extra_paths` for targeted scans that bypass SKIP_DIRS.
pub fn refresh_cache_with_root(
    broad_root: Option<&Path>,
    extra_paths: &[PathBuf],
) -> Result<()> {
    let lockfile = inventory::load_config()?;
    let discovered = discover_unmanaged(&lockfile, broad_root, extra_paths)?;

    let mut cfg = inventory::load_config()?;
    cfg.discovered = discovered;
    cfg.last_scan = Some(chrono::Utc::now().to_rfc3339());
    inventory::save_config(&cfg)?;
    Ok(())
}

/// Discover all unmanaged resources. Pass 1 scans known tool dirs, pass 2 (if `broad_root` set)
/// walks from a root directory looking for SKILL.md and agent .md files anywhere.
/// Pass 3 (if `extra_paths` non-empty) does targeted scans without SKIP_DIRS filtering.
fn discover_unmanaged(
    lockfile: &Config,
    broad_root: Option<&Path>,
    extra_paths: &[PathBuf],
) -> Result<Vec<DiscoveredResource>> {
    let installed = tools::installed_tools();
    let mut found: Vec<DiscoveredResource> = Vec::new();

    // Pass 1: known tool directories
    for tool in &installed {
        if let Some(skills_dir) = tool.skills_dir()
            && skills_dir.exists()
        {
            scan_skills_dir(&skills_dir, tool.as_ref(), lockfile, &mut found)?;
        }

        if let Some(agents_dir) = tool.agents_dir()
            && agents_dir.exists()
        {
            scan_agents_dir(&agents_dir, tool.as_ref(), lockfile, &mut found)?;
        }
    }

    // Also check shared store for untracked items
    scan_shared_store(lockfile, &mut found)?;

    // Scan project roots for configs and llms.txt
    scan_project_configs(lockfile, &mut found);

    // Scan memory files
    scan_memory_files(lockfile, &mut found);

    // Pass 2: broad scan from root (if requested)
    if let Some(root) = broad_root {
        broad_scan(root, lockfile, &mut found)?;
    }

    // Pass 3: targeted scan of user-specified paths (bypasses SKIP_DIRS)
    if !extra_paths.is_empty() {
        targeted_scan(extra_paths, lockfile, &mut found)?;
    }

    Ok(found)
}

/// Directories to skip during broad scan.
const SKIP_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    ".cache",
    ".cargo",
    ".rustup",
    "Library",
    ".Trash",
    ".npm",
    ".local",
    "venv",
    ".venv",
    "__pycache__",
    ".tox",
    "dist",
    "build",
    ".next",
];

/// Check whether a file has valid agent frontmatter (YAML with `name` + `description` fields).
pub fn has_valid_agent_frontmatter(path: &Path) -> bool {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return false,
    };
    let parsed = match frontmatter::parse(&content) {
        Ok(p) => p,
        Err(_) => return false,
    };
    let fm: serde_yaml::Value = match serde_yaml::from_str(&parsed.frontmatter) {
        Ok(v) => v,
        Err(_) => return false,
    };
    let Some(map) = fm.as_mapping() else {
        return false;
    };
    map.contains_key(serde_yaml::Value::String("name".into()))
        && map.contains_key(serde_yaml::Value::String("description".into()))
}

/// Scan entries inside a directory during broad/targeted scan, discovering both
/// SKILL.md files and agent `.md` files inside `agents/` directories.
fn scan_walk_entry(
    entry: &walkdir::DirEntry,
    cfg: &Config,
    found: &mut Vec<DiscoveredResource>,
) {
    let path = entry.path();

    // Look for SKILL.md → parent is a skill directory
    if entry.file_name() == "SKILL.md" && path.is_file() {
        let skill_dir = match path.parent() {
            Some(d) => d,
            None => return,
        };
        if skill_dir.is_symlink() {
            return;
        }
        let name = skill_dir.file_name().unwrap().to_string_lossy().to_string();
        if cfg.find(&name, TrackedKind::Skill).is_some() {
            return;
        }
        if found
            .iter()
            .any(|d| d.name == name && d.kind == TrackedKind::Skill)
        {
            return;
        }
        let content_hash = hash_resource(TrackedKind::Skill, skill_dir).ok();
        found.push(DiscoveredResource {
            name,
            kind: TrackedKind::Skill,
            found_in: vec![DiscoveryLocation {
                tool: "filesystem".to_string(),
                path: skill_dir.to_string_lossy().to_string(),
            }],
            content_hash,
        });
        return;
    }

    // Look for agents/ directories → scan .md files inside for valid agent frontmatter
    if entry.file_type().is_dir() && entry.file_name() == "agents" {
        let entries = match std::fs::read_dir(path) {
            Ok(e) => e,
            Err(_) => return,
        };
        for child in entries.filter_map(|e| e.ok()) {
            let child_path = child.path();
            if child_path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            if child_path.is_symlink() {
                continue;
            }
            let fname = child_path.file_name().unwrap().to_string_lossy();
            if fname == "SKILL.md" || fname.eq_ignore_ascii_case("AGENTS.md") {
                continue;
            }
            if !has_valid_agent_frontmatter(&child_path) {
                continue;
            }
            let name = child_path.file_stem().unwrap().to_string_lossy().to_string();
            if cfg.find(&name, TrackedKind::Agent).is_some() {
                continue;
            }
            if found
                .iter()
                .any(|d| d.name == name && d.kind == TrackedKind::Agent)
            {
                continue;
            }
            let content_hash = inventory::hash_file(&child_path).ok();
            found.push(DiscoveredResource {
                name,
                kind: TrackedKind::Agent,
                found_in: vec![DiscoveryLocation {
                    tool: "filesystem".to_string(),
                    path: child_path.to_string_lossy().to_string(),
                }],
                content_hash,
            });
        }
    }
}

/// Broad scan from a root directory looking for SKILL.md and agent .md files.
fn broad_scan(root: &Path, cfg: &Config, found: &mut Vec<DiscoveredResource>) -> Result<()> {
    // Collect known dirs to skip (already scanned in pass 1)
    let known_dirs: Vec<PathBuf> = {
        let mut dirs = vec![config::shared_skills_dir(), config::shared_agents_dir()];
        for tool in &tools::installed_tools() {
            if let Some(d) = tool.skills_dir() {
                dirs.push(d);
            }
            if let Some(d) = tool.agents_dir() {
                dirs.push(d);
            }
        }
        dirs
    };

    for entry in walkdir::WalkDir::new(root)
        .max_depth(6)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            // Skip noise directories
            if e.file_type().is_dir() && SKIP_DIRS.contains(&name.as_ref()) {
                return false;
            }
            // Skip directories already scanned in pass 1
            if e.file_type().is_dir() && known_dirs.iter().any(|k| e.path() == k) {
                return false;
            }
            true
        })
        .filter_map(|e| e.ok())
    {
        scan_walk_entry(&entry, cfg, found);
    }

    Ok(())
}

/// Targeted scan of user-specified paths. Same discovery logic as broad_scan but
/// without SKIP_DIRS filtering, allowing discovery in paths like `.local`.
fn targeted_scan(
    paths: &[PathBuf],
    cfg: &Config,
    found: &mut Vec<DiscoveredResource>,
) -> Result<()> {
    for scan_root in paths {
        if !scan_root.exists() {
            continue;
        }
        for entry in walkdir::WalkDir::new(scan_root)
            .max_depth(6)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            scan_walk_entry(&entry, cfg, found);
        }
    }
    Ok(())
}

fn scan_skills_dir(
    dir: &Path,
    tool: &dyn CodingTool,
    cfg: &Config,
    found: &mut Vec<DiscoveredResource>,
) -> Result<()> {
    // Convention: skills are {dir}/{name}/SKILL.md — one level deep.
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return Ok(()),
    };

    for entry in entries.filter_map(|e| e.ok()) {
        let skill_dir = entry.path();

        // Skip symlinks (managed) and non-directories
        if skill_dir.is_symlink() || !skill_dir.is_dir() {
            continue;
        }

        if !skill_dir.join("SKILL.md").exists() {
            continue;
        }

        let name = skill_dir.file_name().unwrap().to_string_lossy().to_string();

        if cfg.find(&name, TrackedKind::Skill).is_some() {
            continue;
        }

        let location = DiscoveryLocation {
            tool: tool.slug().to_string(),
            path: skill_dir.to_string_lossy().to_string(),
        };

        let content_hash = hash_resource(TrackedKind::Skill, &skill_dir).ok();

        if let Some(existing) = found
            .iter_mut()
            .find(|d| d.name == name && d.kind == TrackedKind::Skill)
        {
            if !existing.found_in.iter().any(|l| l.tool == tool.slug()) {
                existing.found_in.push(location);
            }
        } else {
            found.push(DiscoveredResource {
                name,
                kind: TrackedKind::Skill,
                found_in: vec![location],
                content_hash,
            });
        }
    }

    Ok(())
}

fn scan_agents_dir(
    dir: &Path,
    tool: &dyn CodingTool,
    cfg: &Config,
    found: &mut Vec<DiscoveredResource>,
) -> Result<()> {
    // Convention: agents are {dir}/{name}.md — flat files.
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return Ok(()),
    };

    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();

        if path.is_symlink() {
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }

        let fname = path.file_name().unwrap().to_string_lossy();
        if fname == "SKILL.md" || fname.eq_ignore_ascii_case("AGENTS.md") {
            continue;
        }

        let name = path.file_stem().unwrap().to_string_lossy().to_string();

        if cfg.find(&name, TrackedKind::Agent).is_some() {
            continue;
        }

        let location = DiscoveryLocation {
            tool: tool.slug().to_string(),
            path: path.to_string_lossy().to_string(),
        };

        let content_hash = inventory::hash_file(&path).ok();

        if let Some(existing) = found
            .iter_mut()
            .find(|d| d.name == name && d.kind == TrackedKind::Agent)
        {
            if !existing.found_in.iter().any(|l| l.tool == tool.slug()) {
                existing.found_in.push(location);
            }
        } else {
            found.push(DiscoveredResource {
                name,
                kind: TrackedKind::Agent,
                found_in: vec![location],
                content_hash,
            });
        }
    }

    Ok(())
}

fn scan_shared_store(lockfile: &Config, found: &mut Vec<DiscoveredResource>) -> Result<()> {
    // Check skills in shared store not in lockfile
    let skills_dir = config::shared_skills_dir();
    if skills_dir.exists() {
        for entry in std::fs::read_dir(&skills_dir)?.filter_map(|e| e.ok()) {
            let path = entry.path();
            if !path.is_dir() || path.is_symlink() {
                continue;
            }
            if !path.join("SKILL.md").exists() {
                continue;
            }
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            if lockfile.find(&name, TrackedKind::Skill).is_some() {
                continue;
            }
            if found
                .iter()
                .any(|d| d.name == name && d.kind == TrackedKind::Skill)
            {
                continue;
            }
            let content_hash = hash_resource(TrackedKind::Skill, &path).ok();
            found.push(DiscoveredResource {
                name,
                kind: TrackedKind::Skill,
                found_in: vec![DiscoveryLocation {
                    tool: "shared-store".to_string(),
                    path: path.to_string_lossy().to_string(),
                }],
                content_hash,
            });
        }
    }

    // Check agents in shared store not in lockfile
    let agents_dir = config::shared_agents_dir();
    if agents_dir.exists() {
        for entry in std::fs::read_dir(&agents_dir)?.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_symlink() {
                continue;
            }
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            let name = path.file_stem().unwrap().to_string_lossy().to_string();
            if lockfile.find(&name, TrackedKind::Agent).is_some() {
                continue;
            }
            if found
                .iter()
                .any(|d| d.name == name && d.kind == TrackedKind::Agent)
            {
                continue;
            }
            let content_hash = inventory::hash_file(&path).ok();
            found.push(DiscoveredResource {
                name,
                kind: TrackedKind::Agent,
                found_in: vec![DiscoveryLocation {
                    tool: "shared-store".to_string(),
                    path: path.to_string_lossy().to_string(),
                }],
                content_hash,
            });
        }
    }

    Ok(())
}

/// Adopt a single discovered resource: move to shared store, update lockfile.
pub fn adopt(
    name: &str,
    kind: TrackedKind,
    tool_slugs: Option<&[String]>,
    all_tools: bool,
    copy: bool,
) -> Result<()> {
    let cfg = inventory::load_config()?;
    let mut lockfile = inventory::load_config()?;

    // Already managed?
    if lockfile.find(name, kind).is_some() {
        return Err(AppError::AlreadyExists(format!(
            "{kind} '{name}' is already managed"
        )));
    }

    // Find in discovery cache
    let discovered = cfg
        .discovered
        .iter()
        .find(|d| d.name == name && d.kind == kind)
        .ok_or_else(|| {
            AppError::Other(format!(
                "{kind} '{name}' not found in discovery cache. Run `agentspec discover` first."
            ))
        })?;

    let source_loc = &discovered.found_in[0];
    let source_path = Path::new(&source_loc.path);

    let resource_kind: ResourceKind = kind.into();

    // Determine if already in shared store or needs copying
    let is_shared_store = source_loc.tool == "shared-store";

    let (relative_path, abs_dest) = match kind {
        TrackedKind::Skill => {
            let dest = config::shared_skills_dir().join(name);
            if !is_shared_store {
                if dest.exists() {
                    return Err(AppError::AlreadyExists(format!(
                        "Skill directory already exists at {}",
                        dest.display()
                    )));
                }
                copy_dir(source_path, &dest)?;
            }
            (format!("skills/{name}"), dest)
        }
        TrackedKind::Agent => {
            let dest = config::shared_agents_dir().join(format!("{name}.md"));
            if !is_shared_store {
                if dest.exists() {
                    return Err(AppError::AlreadyExists(format!(
                        "Agent file already exists at {}",
                        dest.display()
                    )));
                }
                std::fs::copy(source_path, &dest)?;
            }
            (format!("agents/{name}.md"), dest)
        }
        // New entity types — adopt in-place (no copy to shared store)
        _ => {
            let abs = source_path.to_path_buf();
            let relative = source_path.to_string_lossy().to_string();
            (relative, abs)
        }
    };

    let hash = hash_resource(kind, &abs_dest)?;

    // Replace original with symlink (for each found_in location)
    for loc in &discovered.found_in {
        let loc_path = Path::new(&loc.path);
        if loc_path.exists() && !loc_path.is_symlink() && loc.tool != "shared-store" {
            // Remove original, replace with symlink
            if kind == TrackedKind::Skill {
                std::fs::remove_dir_all(loc_path)?;
            } else {
                std::fs::remove_file(loc_path)?;
            }
            let target = link::make_relative_public(loc_path, &abs_dest);
            std::os::unix::fs::symlink(&target, loc_path)?;
        }
    }

    // Build tracked resource
    let mut tracked = TrackedResource::new(
        name.to_string(),
        kind,
        "discovered".to_string(),
        SourceType::Discovered,
        relative_path,
        hash,
    );

    // Record existing links from discovery
    for loc in &discovered.found_in {
        if loc.tool != "shared-store" {
            tracked.links.push(ResourceLink {
                tool: loc.tool.clone(),
                strategy: LinkStrategy::Symlink,
                path: loc.path.clone(),
            });
        }
    }

    lockfile.add(tracked);
    inventory::save_config(&lockfile)?;

    // Link to additional tools if requested
    let slugs = resolve_tool_slugs(tool_slugs, all_tools);
    if !slugs.is_empty() {
        link::link_to_tools(resource_kind, name, &slugs, copy)?;
    }

    println!(
        "  {} Adopted {} '{}'",
        style("✓").green().bold(),
        kind,
        name
    );

    // Remove from discovery cache
    let mut cfg = inventory::load_config()?;
    cfg.discovered
        .retain(|d| !(d.name == name && d.kind == kind));
    inventory::save_config(&cfg)?;

    Ok(())
}

/// Adopt all discovered resources.
pub fn adopt_all(all_tools: bool, copy: bool) -> Result<()> {
    let cfg = inventory::load_config()?;
    let to_adopt: Vec<(String, TrackedKind)> = cfg
        .discovered
        .iter()
        .map(|d| (d.name.clone(), d.kind))
        .collect();

    if to_adopt.is_empty() {
        println!(
            "  {} Nothing to adopt. Run `agentspec discover` first.",
            style("~").yellow()
        );
        return Ok(());
    }

    let tool_slugs: Option<Vec<String>> = if all_tools { None } else { Some(Vec::new()) };

    for (name, kind) in &to_adopt {
        if let Err(e) = adopt(name, *kind, tool_slugs.as_deref(), all_tools, copy) {
            eprintln!(
                "  {} Could not adopt {} '{}': {e}",
                style("✗").red(),
                kind,
                name
            );
        }
    }

    Ok(())
}

/// Show full inventory status from lockfile + config cache.
pub fn status(json: bool) -> Result<()> {
    let lockfile = inventory::load_config()?;
    let cfg = inventory::load_config()?;

    if json {
        let managed: Vec<serde_json::Value> = lockfile
            .resources
            .iter()
            .map(|r| {
                serde_json::json!({
                    "name": r.name,
                    "kind": format!("{}", r.kind),
                    "source": r.source,
                    "source_type": r.source_type,
                    "hash": r.hash,
                    "links": r.links.iter().map(|l| {
                        serde_json::json!({ "tool": l.tool, "strategy": l.strategy })
                    }).collect::<Vec<_>>(),
                })
            })
            .collect();
        let unmanaged: Vec<serde_json::Value> = cfg
            .discovered
            .iter()
            .map(|d| {
                serde_json::json!({
                    "name": d.name,
                    "kind": format!("{}", d.kind),
                    "found_in": d.found_in.iter().map(|l| &l.tool).collect::<Vec<_>>(),
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "managed": managed,
                "unmanaged": unmanaged,
            }))?
        );
        return Ok(());
    }

    // Managed resources
    println!("  {}", style("Managed Resources").bold().underlined());
    if lockfile.resources.is_empty() {
        println!("  (none)");
    } else {
        for r in &lockfile.resources {
            let tools: Vec<&str> = r.links.iter().map(|l| l.tool.as_str()).collect();
            println!(
                "  {} {:<35} {:<15} {}",
                style("●").green(),
                r.name,
                format!("{}", r.kind),
                if tools.is_empty() {
                    "(unlinked)".to_string()
                } else {
                    tools.join(", ")
                }
            );
        }
    }

    // Unmanaged resources
    println!("\n  {}", style("Unmanaged Resources").bold().underlined());
    if cfg.discovered.is_empty() {
        println!("  (none found)");
    } else {
        for d in &cfg.discovered {
            let tools: Vec<&str> = d.found_in.iter().map(|l| l.tool.as_str()).collect();
            println!(
                "  {} {:<35} {:<15} {}",
                style("○").dim(),
                d.name,
                format!("{}", d.kind),
                tools.join(", ")
            );
        }
    }

    Ok(())
}

/// Scan decoded project roots for all project files (AGENTS.md, instruction files, llms.txt).
/// Uses the unified project file registry — no hardcoded filenames.
/// Only considers directories that are git repositories (contain .git).
fn scan_project_configs(cfg: &Config, found: &mut Vec<DiscoveredResource>) {
    let project_infos = memory::scan_project_infos();

    for p in &project_infos {
        let Some(ref pp) = p.project_path else {
            continue;
        };

        // Only scan actual git repositories
        if !pp.join(".git").exists() {
            continue;
        }

        let project_name = pp
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| p.encoded_name.clone());

        // Single loop: discover all project files from the registry
        for (spec, file_path) in project_files::find_in_project(pp) {
            let name = format!("{project_name}/{}", spec.filename);
            if cfg.find(&name, spec.kind).is_none()
                && !found.iter().any(|d| d.name == name)
            {
                let content_hash = if spec.is_directory {
                    inventory::hash_dir(&file_path).ok()
                } else {
                    inventory::hash_file(&file_path).ok()
                };
                found.push(DiscoveredResource {
                    name,
                    kind: spec.kind,
                    found_in: vec![DiscoveryLocation {
                        tool: spec.editor.to_string(),
                        path: file_path.to_string_lossy().to_string(),
                    }],
                    content_hash,
                });
            }
        }
    }

    // Global files (instruction files with global_path set)
    for (spec, file_path) in project_files::find_global() {
        let name = format!("global/{}", spec.filename);
        if cfg.find(&name, spec.kind).is_none()
            && !found.iter().any(|d| d.name == name)
        {
            let content_hash = if spec.is_directory {
                inventory::hash_dir(&file_path).ok()
            } else {
                inventory::hash_file(&file_path).ok()
            };
            found.push(DiscoveredResource {
                name,
                kind: spec.kind,
                found_in: vec![DiscoveryLocation {
                    tool: spec.editor.to_string(),
                    path: file_path.to_string_lossy().to_string(),
                }],
                content_hash,
            });
        }
    }
}

/// Scan Claude Code memory files as discoverable entities.
fn scan_memory_files(cfg: &Config, found: &mut Vec<DiscoveredResource>) {
    let memories = memory::scan_memories();

    for m in &memories {
        let project_name = m
            .project_path
            .as_deref()
            .and_then(|p| {
                std::path::Path::new(p)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
            })
            .unwrap_or_else(|| m.project_name.clone());

        let name = format!("{project_name}/{}", m.name);
        if cfg.find(&name, TrackedKind::Memory).is_none() && !found.iter().any(|d| d.name == name) {
            found.push(DiscoveredResource {
                name,
                kind: TrackedKind::Memory,
                found_in: vec![DiscoveryLocation {
                    tool: "claude-code".to_string(),
                    path: m.file_path.to_string_lossy().to_string(),
                }],
                content_hash: inventory::hash_file(&m.file_path).ok(),
            });
        }
    }
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
