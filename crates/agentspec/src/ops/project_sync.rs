use std::collections::HashMap;
use std::path::{Path, PathBuf};

use console::style;

use crate::config;
use crate::error::{AppError, Result};
use crate::inventory::{Config, TrackedProject, hash_dir, hash_file};
use crate::ops::manage::copy_dir;
use crate::ops::memory;
use crate::project_files;

// ---------------------------------------------------------------------------
// Sync a single project
// ---------------------------------------------------------------------------

/// Outcome of syncing one project, for reporting.
struct SyncOutcome {
    key: String,
    path: PathBuf,
    synced_files: Vec<&'static str>,
}

/// Sync a project: copy instruction files, configs, memories into ~/.agents/projects/{key}/.
/// Enables auto-sync. Source files are never modified or deleted.
pub fn sync_project(cfg: &mut Config, name: &str, json: bool) -> Result<()> {
    // Find project path — check existing tracked project or discover from known projects
    let project_path = if let Some(p) = find_tracked(cfg, name)? {
        PathBuf::from(&p.path)
    } else {
        find_project_path(name)?
    };

    let outcome = sync_project_at(cfg, &project_path)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&outcome_json(&outcome))?);
    } else {
        print_outcome(&outcome);
    }

    Ok(())
}

fn sync_project_at(cfg: &mut Config, project_path: &Path) -> Result<SyncOutcome> {
    if !project_path.exists() {
        return Err(AppError::Other(format!(
            "Project path does not exist: {}",
            project_path.display()
        )));
    }

    let key = project_key(project_path);
    let dest_dir = config::shared_project_dir(&key);
    std::fs::create_dir_all(&dest_dir)?;

    let mut source_hashes = HashMap::new();
    let mut synced_files = Vec::new();

    // Sync all project files from registry (AGENTS.md, llms.txt, instruction files)
    for (spec, file_path) in project_files::find_in_project(project_path) {
        let dest = dest_dir.join(spec.filename);
        if spec.is_directory {
            if dest.exists() {
                std::fs::remove_dir_all(&dest)?;
            }
            copy_dir(&file_path, &dest)?;
            let hash = hash_dir(&file_path)?;
            source_hashes.insert(spec.filename.to_string(), hash);
        } else {
            std::fs::copy(&file_path, &dest)?;
            let hash = hash_file(&file_path)?;
            source_hashes.insert(spec.filename.to_string(), hash);
        }
        synced_files.push(spec.filename);
    }

    // Sync memories from Claude Code project dirs
    sync_project_memories(&key, project_path, &dest_dir)?;

    let now = chrono::Utc::now().to_rfc3339();
    let path_str = project_path.to_string_lossy().to_string();

    // Drop any entry tracking the same path under a stale (pre-encoding) key.
    cfg.projects
        .retain(|p| !(p.path == path_str && p.name != key));

    let tracked = TrackedProject {
        name: key.clone(),
        path: path_str,
        sync: true,
        synced_at: Some(now),
        config_hash: source_hashes.get("AGENTS.md").cloned(),
        source_hashes,
    };

    cfg.add_project(tracked);

    Ok(SyncOutcome {
        key,
        path: project_path.to_path_buf(),
        synced_files,
    })
}

fn outcome_json(o: &SyncOutcome) -> serde_json::Value {
    serde_json::json!({
        "project": o.key,
        "path": o.path.to_string_lossy(),
        "synced_files": o.synced_files,
    })
}

fn print_outcome(o: &SyncOutcome) {
    let display = o
        .path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| o.key.clone());
    println!(
        "  {} Synced project '{}' ({} file(s))",
        style("✓").green().bold(),
        display,
        o.synced_files.len()
    );
    for f in &o.synced_files {
        println!("    {} {f}", style("+").green());
    }
}

// ---------------------------------------------------------------------------
// Desync a project
// ---------------------------------------------------------------------------

/// Stop auto-sync for a project. Copy stays in ~/.agents/projects/{key}/ but goes stale.
/// Source files are never modified or deleted.
pub fn desync_project(cfg: &mut Config, name: &str, json: bool) -> Result<()> {
    let key = find_tracked(cfg, name)?
        .map(|p| p.name.clone())
        .ok_or_else(|| AppError::Other(format!("Project '{name}' is not synced")))?;

    let project = cfg.find_project_mut(&key).unwrap();
    project.sync = false;
    let display = display_name(project);

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "project": key,
                "action": "desynced",
            }))?
        );
    } else {
        println!(
            "  {} Desynced project '{}' (copy stays, auto-sync disabled)",
            style("~").yellow(),
            display
        );
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Remove a project's synced copy
// ---------------------------------------------------------------------------

/// Remove the synced copy from ~/.agents/projects/{key}/.
/// Source files are NEVER modified or deleted.
pub fn remove_synced_project(cfg: &mut Config, name: &str, json: bool) -> Result<()> {
    let (key, display) = match find_tracked(cfg, name)? {
        Some(p) => (p.name.clone(), display_name(p)),
        None => return Err(AppError::Other(format!("Project '{name}' is not tracked"))),
    };

    let dest_dir = config::shared_project_dir(&key);
    if dest_dir.exists() {
        std::fs::remove_dir_all(&dest_dir)?;
    }

    cfg.remove_project(&key);

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "project": key,
                "action": "removed",
            }))?
        );
    } else {
        println!(
            "  {} Removed synced copy of project '{}' (originals untouched)",
            style("✓").green().bold(),
            display
        );
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Resync all projects with sync=true
// ---------------------------------------------------------------------------

/// Re-sync all projects that have auto-sync enabled.
/// Compares hashes, re-copies changed files. Source always wins.
/// Returns (projects resynced, files updated); the caller owns JSON reporting.
pub fn resync_all(cfg: &mut Config, json: bool) -> Result<(usize, usize)> {
    let sync_projects: Vec<String> = cfg
        .projects
        .iter()
        .filter(|p| p.sync)
        .map(|p| p.name.clone())
        .collect();

    if sync_projects.is_empty() {
        if !json {
            println!(
                "  {} No projects with auto-sync enabled",
                style("~").yellow()
            );
        }
        return Ok((0, 0));
    }

    let mut total_updated = 0;
    for name in &sync_projects {
        match resync_project(cfg, name) {
            Ok(updated) => total_updated += updated,
            Err(e) => {
                eprintln!("  {} Could not resync '{}': {e}", style("✗").red(), name);
            }
        }
    }

    if !json {
        println!(
            "  {} Resynced {} project(s), {} file(s) updated",
            style("✓").green().bold(),
            sync_projects.len(),
            total_updated
        );
    }

    Ok((sync_projects.len(), total_updated))
}

/// Resync a single project. Returns the number of files updated.
fn resync_project(cfg: &mut Config, name: &str) -> Result<usize> {
    let project = cfg
        .find_project(name)
        .ok_or_else(|| AppError::Other(format!("Project '{name}' not found")))?
        .clone();

    let project_path = PathBuf::from(&project.path);
    if !project_path.exists() {
        return Err(AppError::Other(format!(
            "Project path no longer exists: {}",
            project_path.display()
        )));
    }

    let dest_dir = config::shared_project_dir(name);
    std::fs::create_dir_all(&dest_dir)?;

    let mut updated = 0;
    let mut new_hashes = project.source_hashes.clone();

    // Check all project files from registry — single loop, no hardcoded names
    for (spec, file_path) in project_files::find_in_project(&project_path) {
        let current_hash = if spec.is_directory {
            hash_dir(&file_path)?
        } else {
            hash_file(&file_path)?
        };

        if project.source_hashes.get(spec.filename) != Some(&current_hash) {
            let dest = dest_dir.join(spec.filename);
            if spec.is_directory {
                if dest.exists() {
                    std::fs::remove_dir_all(&dest)?;
                }
                copy_dir(&file_path, &dest)?;
            } else {
                std::fs::copy(&file_path, &dest)?;
            }
            new_hashes.insert(spec.filename.to_string(), current_hash);
            updated += 1;
        }
    }

    // Re-sync memories
    sync_project_memories(name, &project_path, &dest_dir)?;

    if updated > 0 {
        let project_mut = cfg.find_project_mut(name).unwrap();
        project_mut.source_hashes = new_hashes;
        project_mut.synced_at = Some(chrono::Utc::now().to_rfc3339());
        project_mut.config_hash = project_mut.source_hashes.get("AGENTS.md").cloned();
    }

    Ok(updated)
}

// ---------------------------------------------------------------------------
// Sync all discovered projects at once
// ---------------------------------------------------------------------------

/// Sync all discovered projects.
pub fn sync_all(cfg: &mut Config, json: bool) -> Result<()> {
    let project_infos = memory::scan_project_infos();
    let mut reports = Vec::new();
    let mut synced = 0;

    for p in &project_infos {
        let Some(ref pp) = p.project_path else {
            continue;
        };
        if !pp.join(".git").exists() {
            continue;
        }

        match sync_project_at(cfg, pp) {
            Ok(outcome) => {
                synced += 1;
                if json {
                    reports.push(outcome_json(&outcome));
                } else {
                    print_outcome(&outcome);
                }
            }
            Err(e) => {
                eprintln!(
                    "  {} Could not sync '{}': {e}",
                    style("✗").red(),
                    pp.display()
                );
            }
        }
    }

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::Value::Array(reports))?
        );
    } else {
        println!(
            "\n  {} Synced {} project(s)",
            style("✓").green().bold(),
            synced
        );
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Project status
// ---------------------------------------------------------------------------

/// Show status for a specific project or all projects.
pub fn project_status(cfg: &Config, project_name: Option<&str>, json: bool) -> Result<()> {
    if let Some(name) = project_name {
        let project = find_tracked(cfg, name)?
            .ok_or_else(|| AppError::Other(format!("Project '{name}' is not synced")))?;

        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "name": project.name,
                    "path": project.path,
                    "sync": project.sync,
                    "synced_at": project.synced_at,
                    "files": project.source_hashes.keys().collect::<Vec<_>>(),
                }))?
            );
        } else {
            println!("  {}", style(display_name(project)).bold().underlined());
            println!("  Path: {}", project.path);
            println!(
                "  Auto-sync: {}",
                if project.sync {
                    style("enabled").green()
                } else {
                    style("disabled").yellow()
                }
            );
            if let Some(ref at) = project.synced_at {
                println!("  Last synced: {at}");
            }
            println!("  Files:");
            for (filename, hash) in &project.source_hashes {
                let short_hash = &hash[..hash.len().min(20)];
                println!("    {} {filename} ({short_hash}...)", style("•").dim());
            }
        }
        return Ok(());
    }

    // All projects overview
    let synced: Vec<&TrackedProject> = cfg.projects.iter().filter(|p| p.sync).collect();
    let desynced: Vec<&TrackedProject> = cfg.projects.iter().filter(|p| !p.sync).collect();

    // Discover projects not yet synced (reported by path, which is unambiguous)
    let project_infos = memory::scan_project_infos();
    let discovered_paths: Vec<String> = project_infos
        .iter()
        .filter_map(|p| {
            let pp = p.project_path.as_ref()?;
            if !pp.join(".git").exists() {
                return None;
            }
            let path = pp.to_string_lossy().to_string();
            if cfg.projects.iter().any(|tp| tp.path == path) {
                None
            } else {
                Some(path)
            }
        })
        .collect();

    if json {
        let report = serde_json::json!({
            "synced": synced.iter().map(|p| &p.name).collect::<Vec<_>>(),
            "desynced": desynced.iter().map(|p| &p.name).collect::<Vec<_>>(),
            "discovered": discovered_paths,
        });
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }

    println!("  {}", style("Synced Projects").bold().underlined());
    if synced.is_empty() {
        println!("  (none)");
    } else {
        for p in &synced {
            let file_count = p.source_hashes.len();
            println!(
                "  {} {:<30} {} file(s)  {}",
                style("●").green(),
                display_name(p),
                file_count,
                style(&p.path).dim()
            );
        }
    }

    if !desynced.is_empty() {
        println!("\n  {}", style("Desynced Projects").bold().underlined());
        for p in &desynced {
            println!(
                "  {} {:<30} (stale)  {}",
                style("○").yellow(),
                display_name(p),
                style(&p.path).dim()
            );
        }
    }

    if !discovered_paths.is_empty() {
        println!(
            "\n  {}",
            style("Discovered (not synced)").bold().underlined()
        );
        for path in &discovered_paths {
            println!("  {} {path}", style("○").dim());
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// The shared-store key for a project. Encodes the full path the way Claude
/// Code encodes project dirs (`/Users/u/work/app` -> `-Users-u-work-app`),
/// which stays unique when two projects share a directory basename — the same
/// collision ops/memory.rs guards against.
fn project_key(path: &Path) -> String {
    path.to_string_lossy().replace(['/', '\\'], "-")
}

/// Human-facing project name: the directory basename, with the store key as
/// a fallback.
fn display_name(p: &TrackedProject) -> String {
    Path::new(&p.path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| p.name.clone())
}

/// Resolve a user-supplied identifier (store key, path, or directory basename)
/// to a tracked project. Errors when a bare basename matches more than one
/// tracked project.
fn find_tracked<'a>(cfg: &'a Config, ident: &str) -> Result<Option<&'a TrackedProject>> {
    if let Some(p) = cfg.find_project(ident) {
        return Ok(Some(p));
    }

    let matches: Vec<&TrackedProject> = cfg
        .projects
        .iter()
        .filter(|p| {
            p.path == ident
                || Path::new(&p.path)
                    .file_name()
                    .is_some_and(|n| n.to_string_lossy() == ident)
        })
        .collect();

    match matches.as_slice() {
        [] => Ok(None),
        [one] => Ok(Some(one)),
        many => Err(AppError::Other(format!(
            "Project '{ident}' is ambiguous; use a full path: {}",
            many.iter()
                .map(|p| p.path.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ))),
    }
}

/// Find a project path by name, searching known project directories.
/// Errors when a bare basename matches more than one discoverable project.
fn find_project_path(name: &str) -> Result<PathBuf> {
    let project_infos = memory::scan_project_infos();

    let matches: Vec<&PathBuf> = project_infos
        .iter()
        .filter_map(|p| p.project_path.as_ref())
        .filter(|pp| pp.file_name().is_some_and(|n| n.to_string_lossy() == name))
        .collect();

    match matches.as_slice() {
        [one] => return Ok((*one).clone()),
        [] => {}
        many => {
            return Err(AppError::Other(format!(
                "Project '{name}' is ambiguous; use a full path: {}",
                many.iter()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        }
    }

    // Maybe it's an absolute path
    let as_path = PathBuf::from(name);
    if as_path.is_absolute() && as_path.exists() {
        return Ok(as_path);
    }

    // Try relative to cwd
    let cwd = std::env::current_dir()?;
    let relative = cwd.join(name);
    if relative.exists() {
        return Ok(relative);
    }

    Err(AppError::Other(format!(
        "Could not find project '{name}'. Provide an absolute path or ensure it's discoverable."
    )))
}

/// Copy project-specific memory files into the synced project directory.
fn sync_project_memories(_project_name: &str, project_path: &Path, dest_dir: &Path) -> Result<()> {
    let memories = memory::scan_memories();
    let project_path_str = project_path.to_string_lossy().to_string();

    let project_memories: Vec<&memory::MemoryEntry> = memories
        .iter()
        .filter(|m| {
            m.project_path
                .as_deref()
                .is_some_and(|p| p == project_path_str)
        })
        .collect();

    if project_memories.is_empty() {
        return Ok(());
    }

    let mem_dest = dest_dir.join("memories");
    std::fs::create_dir_all(&mem_dest)?;

    for m in &project_memories {
        let dest_file = mem_dest.join(format!("{}.md", m.name));
        std::fs::copy(&m.file_path, &dest_file)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tracked(key: &str, path: &str) -> TrackedProject {
        TrackedProject {
            name: key.into(),
            path: path.into(),
            sync: true,
            synced_at: None,
            config_hash: None,
            source_hashes: HashMap::new(),
        }
    }

    #[test]
    fn project_key_distinguishes_same_basename() {
        // Two projects that share a folder basename ("app") must NOT collide:
        // the key is the encoded full path, not the basename.
        let a = project_key(Path::new("/Users/urmzd/work/app"));
        let b = project_key(Path::new("/Users/urmzd/personal/app"));
        assert_eq!(a, "-Users-urmzd-work-app");
        assert_ne!(a, b);
    }

    #[test]
    fn find_tracked_resolves_key_path_and_basename() {
        let mut cfg = Config::empty();
        cfg.add_project(tracked("-w-app", "/w/app"));

        assert_eq!(
            find_tracked(&cfg, "-w-app").unwrap().unwrap().path,
            "/w/app"
        );
        assert_eq!(
            find_tracked(&cfg, "/w/app").unwrap().unwrap().name,
            "-w-app"
        );
        assert_eq!(find_tracked(&cfg, "app").unwrap().unwrap().name, "-w-app");
        assert!(find_tracked(&cfg, "missing").unwrap().is_none());

        // A second project with the same basename makes the bare name ambiguous
        // but full paths and keys still resolve.
        cfg.add_project(tracked("-p-app", "/p/app"));
        assert!(find_tracked(&cfg, "app").is_err());
        assert_eq!(
            find_tracked(&cfg, "/p/app").unwrap().unwrap().name,
            "-p-app"
        );
        assert_eq!(
            find_tracked(&cfg, "-w-app").unwrap().unwrap().path,
            "/w/app"
        );
    }
}
