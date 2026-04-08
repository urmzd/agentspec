use std::collections::HashMap;
use std::path::{Path, PathBuf};

use console::style;

use crate::config;
use crate::error::{AppError, Result};
use crate::inventory::{self, TrackedProject, hash_dir, hash_file};
use crate::ops::memory;
use crate::project_files;

// ---------------------------------------------------------------------------
// Sync a single project
// ---------------------------------------------------------------------------

/// Sync a project: copy instruction files, configs, memories into ~/.agents/projects/{name}/.
/// Enables auto-sync. Source files are never modified or deleted.
pub fn sync_project(name: &str, json: bool) -> Result<()> {
    let mut cfg = inventory::load_config()?;

    // Find project path — check existing tracked project or discover from known projects
    let project_path = if let Some(p) = cfg.find_project(name) {
        PathBuf::from(&p.path)
    } else {
        find_project_path(name)?
    };

    if !project_path.exists() {
        return Err(AppError::Other(format!(
            "Project path does not exist: {}",
            project_path.display()
        )));
    }

    let dest_dir = config::shared_project_dir(name);
    std::fs::create_dir_all(&dest_dir)?;

    let mut source_hashes = HashMap::new();
    let mut synced_files = Vec::new();

    // Sync all project files from registry (AGENTS.md, llms.txt, instruction files)
    for (spec, file_path) in project_files::find_in_project(&project_path) {
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
    sync_project_memories(name, &project_path, &dest_dir)?;

    let now = chrono::Utc::now().to_rfc3339();

    let tracked = TrackedProject {
        name: name.to_string(),
        path: project_path.to_string_lossy().to_string(),
        sync: true,
        synced_at: Some(now),
        config_hash: source_hashes.get("AGENTS.md").cloned(),
        source_hashes,
    };

    cfg.add_project(tracked);
    inventory::save_config(&cfg)?;

    if json {
        let report = serde_json::json!({
            "project": name,
            "path": project_path.to_string_lossy(),
            "synced_files": synced_files,
        });
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!(
            "  {} Synced project '{}' ({} file(s))",
            style("✓").green().bold(),
            name,
            synced_files.len()
        );
        for f in &synced_files {
            println!("    {} {f}", style("+").green());
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Desync a project
// ---------------------------------------------------------------------------

/// Stop auto-sync for a project. Copy stays in ~/.agents/projects/{name}/ but goes stale.
/// Source files are never modified or deleted.
pub fn desync_project(name: &str, json: bool) -> Result<()> {
    let mut cfg = inventory::load_config()?;

    let project = cfg
        .find_project_mut(name)
        .ok_or_else(|| AppError::Other(format!("Project '{name}' is not synced")))?;

    project.sync = false;
    inventory::save_config(&cfg)?;

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "project": name,
                "action": "desynced",
            }))?
        );
    } else {
        println!(
            "  {} Desynced project '{}' (copy stays, auto-sync disabled)",
            style("~").yellow(),
            name
        );
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Remove a project's synced copy
// ---------------------------------------------------------------------------

/// Remove the synced copy from ~/.agents/projects/{name}/.
/// Source files are NEVER modified or deleted.
pub fn remove_synced_project(name: &str, json: bool) -> Result<()> {
    let mut cfg = inventory::load_config()?;

    if cfg.find_project(name).is_none() {
        return Err(AppError::Other(format!("Project '{name}' is not tracked")));
    }

    let dest_dir = config::shared_project_dir(name);
    if dest_dir.exists() {
        std::fs::remove_dir_all(&dest_dir)?;
    }

    cfg.remove_project(name);
    inventory::save_config(&cfg)?;

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "project": name,
                "action": "removed",
            }))?
        );
    } else {
        println!(
            "  {} Removed synced copy of project '{}' (originals untouched)",
            style("✓").green().bold(),
            name
        );
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Resync all projects with sync=true
// ---------------------------------------------------------------------------

/// Re-sync all projects that have auto-sync enabled.
/// Compares hashes, re-copies changed files. Source always wins.
pub fn resync_all(json: bool) -> Result<()> {
    let cfg = inventory::load_config()?;
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
        return Ok(());
    }

    let mut total_updated = 0;
    for name in &sync_projects {
        match resync_project(name) {
            Ok(updated) => total_updated += updated,
            Err(e) => {
                if !json {
                    eprintln!("  {} Could not resync '{}': {e}", style("✗").red(), name);
                }
            }
        }
    }

    if json {
        let report = serde_json::json!({
            "projects_resynced": sync_projects.len(),
            "files_updated": total_updated,
        });
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!(
            "  {} Resynced {} project(s), {} file(s) updated",
            style("✓").green().bold(),
            sync_projects.len(),
            total_updated
        );
    }

    Ok(())
}

/// Resync a single project. Returns the number of files updated.
fn resync_project(name: &str) -> Result<usize> {
    let mut cfg = inventory::load_config()?;
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
        inventory::save_config(&cfg)?;
    }

    Ok(updated)
}

// ---------------------------------------------------------------------------
// Sync all discovered projects at once
// ---------------------------------------------------------------------------

/// Sync all discovered projects.
pub fn sync_all(json: bool) -> Result<()> {
    let project_infos = memory::scan_project_infos();
    let mut synced = 0;

    for p in &project_infos {
        let Some(ref pp) = p.project_path else {
            continue;
        };
        if !pp.join(".git").exists() {
            continue;
        }
        let name = pp
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| p.encoded_name.clone());

        if let Err(e) = sync_project(&name, json) {
            if !json {
                eprintln!("  {} Could not sync '{}': {e}", style("✗").red(), name);
            }
        } else {
            synced += 1;
        }
    }

    if !json {
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
pub fn project_status(project_name: Option<&str>, json: bool) -> Result<()> {
    let cfg = inventory::load_config()?;

    if let Some(name) = project_name {
        let project = cfg
            .find_project(name)
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
            println!("  {}", style(&project.name).bold().underlined());
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

    // Discover projects not yet synced
    let project_infos = memory::scan_project_infos();
    let discovered_names: Vec<String> = project_infos
        .iter()
        .filter_map(|p| {
            let pp = p.project_path.as_ref()?;
            if !pp.join(".git").exists() {
                return None;
            }
            let name = pp
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| p.encoded_name.clone());
            if cfg.find_project(&name).is_none() {
                Some(name)
            } else {
                None
            }
        })
        .collect();

    if json {
        let report = serde_json::json!({
            "synced": synced.iter().map(|p| &p.name).collect::<Vec<_>>(),
            "desynced": desynced.iter().map(|p| &p.name).collect::<Vec<_>>(),
            "discovered": discovered_names,
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
                "  {} {:<30} {} file(s)",
                style("●").green(),
                p.name,
                file_count
            );
        }
    }

    if !desynced.is_empty() {
        println!("\n  {}", style("Desynced Projects").bold().underlined());
        for p in &desynced {
            println!("  {} {:<30} (stale)", style("○").yellow(), p.name,);
        }
    }

    if !discovered_names.is_empty() {
        println!(
            "\n  {}",
            style("Discovered (not synced)").bold().underlined()
        );
        for name in &discovered_names {
            println!("  {} {name}", style("○").dim());
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Find a project path by name, searching known project directories.
fn find_project_path(name: &str) -> Result<PathBuf> {
    let project_infos = memory::scan_project_infos();

    for p in &project_infos {
        let Some(ref pp) = p.project_path else {
            continue;
        };
        let dir_name = pp
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        if dir_name == name {
            return Ok(pp.clone());
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
