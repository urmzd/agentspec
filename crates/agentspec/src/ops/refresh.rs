//! `manage update [name]` — re-pull git/local-sourced resources and refresh
//! their stored hashes without removing and re-adding them.
//!
//! Git-sourced resources are re-cloned from their recorded `source`; local
//! resources are re-copied from their recorded path. Discovered resources have
//! no upstream to pull from and are skipped. After a refresh the SHA-256 hash
//! (the lock) and `updated_at` are updated, and any copy-strategy links are
//! re-propagated so tool directories stay in sync.

use std::path::{Path, PathBuf};

use console::style;
use serde::Serialize;
use walkdir::WalkDir;

use crate::config;
use crate::error::{AppError, Result};
use crate::inventory::{
    Config, LinkStrategy, SourceType, TrackedKind, TrackedResource, hash_resource,
};
use crate::ops::discover;
use crate::ops::manage::{self, SourceKind};

#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum UpdateOutcome {
    Updated {
        name: String,
        kind: String,
        old_hash: String,
        new_hash: String,
    },
    Unchanged {
        name: String,
        kind: String,
    },
    Skipped {
        name: String,
        reason: String,
    },
    Failed {
        name: String,
        error: String,
    },
}

/// Update a single managed resource by name (across any kind it's tracked as).
pub fn update_resource(cfg: &mut Config, name: &str, json: bool) -> Result<()> {
    let targets: Vec<(String, TrackedKind)> = cfg
        .resources
        .iter()
        .filter(|r| r.name == name)
        .map(|r| (r.name.clone(), r.kind))
        .collect();

    if targets.is_empty() {
        return Err(AppError::Other(format!(
            "no managed resource named '{name}'"
        )));
    }

    let outcomes = refresh_targets(cfg, &targets);
    report(&outcomes, json);
    Ok(())
}

/// Update all managed resources.
pub fn update_all(cfg: &mut Config, json: bool) -> Result<()> {
    let targets: Vec<(String, TrackedKind)> = cfg
        .resources
        .iter()
        .map(|r| (r.name.clone(), r.kind))
        .collect();

    if targets.is_empty() {
        if json {
            println!("[]");
        } else {
            println!("  no managed resources to update");
        }
        return Ok(());
    }

    let outcomes = refresh_targets(cfg, &targets);
    report(&outcomes, json);
    Ok(())
}

/// Refresh every Local-sourced resource from its origin. Used by the sync
/// pipeline so resources adopted from a repo flow origin → shared store →
/// copy links on every sync. Returns (updated, failed) counts.
pub fn update_local_sources(cfg: &mut Config) -> (usize, usize) {
    let targets: Vec<(String, TrackedKind)> = cfg
        .resources
        .iter()
        .filter(|r| matches!(r.source_type, SourceType::Local))
        .map(|r| (r.name.clone(), r.kind))
        .collect();

    let outcomes = refresh_targets(cfg, &targets);
    let updated = outcomes
        .iter()
        .filter(|o| matches!(o, UpdateOutcome::Updated { .. }))
        .count();
    let failed = outcomes
        .iter()
        .filter(|o| matches!(o, UpdateOutcome::Failed { .. }))
        .count();
    (updated, failed)
}

fn refresh_targets(cfg: &mut Config, targets: &[(String, TrackedKind)]) -> Vec<UpdateOutcome> {
    let mut outcomes = Vec::with_capacity(targets.len());
    for (name, kind) in targets {
        // Re-borrow each iteration so we can mutate the matched resource.
        let outcome = match cfg.find(name, *kind).cloned() {
            Some(mut resource) => {
                let outcome = refresh_one(&mut resource);
                // Persist any field changes (hash / updated_at) back into config.
                if let Some(slot) = cfg.find_mut(name, *kind) {
                    *slot = resource;
                }
                outcome
            }
            None => UpdateOutcome::Failed {
                name: name.clone(),
                error: "resource vanished from config".into(),
            },
        };
        outcomes.push(outcome);
    }
    outcomes
}

fn refresh_one(resource: &mut TrackedResource) -> UpdateOutcome {
    match resource.source_type {
        SourceType::Discovered => UpdateOutcome::Skipped {
            name: resource.name.clone(),
            reason: "discovered resource has no upstream to pull from".into(),
        },
        SourceType::Git => refresh_git(resource),
        SourceType::Local => refresh_local(resource),
    }
}

fn refresh_git(resource: &mut TrackedResource) -> UpdateOutcome {
    match manage::resolve_source(&resource.source) {
        SourceKind::Git(gs) => match manage::clone_source_to_tempdir(&gs) {
            Ok((_tmp, install_dir)) => apply_update(resource, &install_dir),
            Err(e) => UpdateOutcome::Failed {
                name: resource.name.clone(),
                error: format!("{e}"),
            },
        },
        // Legacy entries whose source now resolves as a local path.
        SourceKind::Local(_) => refresh_local(resource),
    }
}

fn refresh_local(resource: &mut TrackedResource) -> UpdateOutcome {
    let src_root = PathBuf::from(&resource.source);
    if !src_root.exists() {
        return UpdateOutcome::Skipped {
            name: resource.name.clone(),
            reason: format!("source path no longer exists: {}", resource.source),
        };
    }
    apply_update(resource, &src_root)
}

/// Locate the resource within `src_root`, copy it over the shared-store path,
/// recompute the hash, and re-propagate copy-strategy links.
fn apply_update(resource: &mut TrackedResource, src_root: &Path) -> UpdateOutcome {
    let dest = config::agents_base_dir().join(&resource.path);

    let src = match resource.kind {
        TrackedKind::Skill => find_skill_src(src_root, &resource.name),
        TrackedKind::Agent => find_agent_src(src_root, &resource.name),
        other => {
            return UpdateOutcome::Skipped {
                name: resource.name.clone(),
                reason: format!("update not supported for {other} resources"),
            };
        }
    };

    let Some(src) = src else {
        return UpdateOutcome::Failed {
            name: resource.name.clone(),
            error: format!(
                "could not locate '{}' in {}",
                resource.name,
                src_root.display()
            ),
        };
    };

    if let Err(e) = copy_over(resource.kind, &src, &dest) {
        return UpdateOutcome::Failed {
            name: resource.name.clone(),
            error: format!("copy failed: {e}"),
        };
    }

    let new_hash = match hash_resource(resource.kind, &dest) {
        Ok(h) => h,
        Err(e) => {
            return UpdateOutcome::Failed {
                name: resource.name.clone(),
                error: format!("hash failed: {e}"),
            };
        }
    };

    if new_hash == resource.hash {
        return UpdateOutcome::Unchanged {
            name: resource.name.clone(),
            kind: resource.kind.to_string(),
        };
    }

    let old_hash = std::mem::replace(&mut resource.hash, new_hash.clone());
    resource.updated_at = chrono::Utc::now().to_rfc3339();

    if let Err(e) = propagate_copy_links(resource, &dest) {
        eprintln!(
            "  warning: failed to refresh copy links for {}: {e}",
            resource.name
        );
    }

    UpdateOutcome::Updated {
        name: resource.name.clone(),
        kind: resource.kind.to_string(),
        old_hash,
        new_hash,
    }
}

/// Copy a resource from `src` over `dest`, replacing what's there.
///
/// For skill directories the copy is staged into a sibling temp dir and then
/// renamed into place, so a mid-copy failure never destroys the existing
/// canonical copy (rename is atomic on a single filesystem).
fn copy_over(kind: TrackedKind, src: &Path, dest: &Path) -> Result<()> {
    match kind {
        TrackedKind::Skill => {
            let fname = dest
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "skill".into());
            let staging = dest.with_file_name(format!(".{fname}.agentspec-tmp"));
            if staging.exists() {
                std::fs::remove_dir_all(&staging)?;
            }
            manage::copy_dir(src, &staging)?;
            if dest.exists() {
                std::fs::remove_dir_all(dest)?;
            }
            std::fs::rename(&staging, dest)?;
        }
        _ => {
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(src, dest)?;
        }
    }
    Ok(())
}

/// Re-copy the refreshed resource to every copy-strategy link. Symlinks point
/// into the shared store and need no update.
fn propagate_copy_links(resource: &TrackedResource, dest: &Path) -> Result<()> {
    for link in &resource.links {
        if link.strategy != LinkStrategy::Copy {
            continue;
        }
        let link_path = PathBuf::from(&link.path);
        copy_over(resource.kind, dest, &link_path)?;
    }
    Ok(())
}

/// Find a skill directory (containing SKILL.md) named `name` within `root`.
fn find_skill_src(root: &Path, name: &str) -> Option<PathBuf> {
    if root.join("SKILL.md").exists() && root.file_name().is_some_and(|n| n == name) {
        return Some(root.to_path_buf());
    }
    for entry in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
        if entry.file_name() != "SKILL.md" {
            continue;
        }
        let dir = entry.path().parent()?;
        if dir.file_name().is_some_and(|n| n == name) {
            return Some(dir.to_path_buf());
        }
    }
    None
}

/// Find an agent markdown file named `name` within `root`.
fn find_agent_src(root: &Path, name: &str) -> Option<PathBuf> {
    let is_match = |p: &Path| {
        p.extension().and_then(|e| e.to_str()) == Some("md")
            && p.file_stem().and_then(|s| s.to_str()) == Some(name)
            && discover::has_valid_agent_frontmatter(p)
    };

    if root.is_file() && is_match(root) {
        return Some(root.to_path_buf());
    }
    for entry in WalkDir::new(root)
        .max_depth(3)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.file_name().and_then(|n| n.to_str()) == Some("SKILL.md") {
            continue;
        }
        if is_match(path) {
            return Some(path.to_path_buf());
        }
    }
    None
}

fn report(outcomes: &[UpdateOutcome], json: bool) {
    if json {
        let out = serde_json::to_string_pretty(outcomes).unwrap_or_else(|_| "[]".into());
        println!("{out}");
        return;
    }

    let mut updated = 0;
    let mut unchanged = 0;
    let mut skipped = 0;
    let mut failed = 0;

    for outcome in outcomes {
        match outcome {
            UpdateOutcome::Updated { name, kind, .. } => {
                println!("  {} updated {name} ({kind})", style("✓").green().bold());
                updated += 1;
            }
            UpdateOutcome::Unchanged { name, .. } => {
                println!("  {} {name} already up to date", style("·").dim());
                unchanged += 1;
            }
            UpdateOutcome::Skipped { name, reason } => {
                println!("  {} skipped {name}: {reason}", style("~").yellow());
                skipped += 1;
            }
            UpdateOutcome::Failed { name, error } => {
                println!("  {} failed {name}: {error}", style("✗").red().bold());
                failed += 1;
            }
        }
    }

    println!("\n  {updated} updated, {unchanged} unchanged, {skipped} skipped, {failed} failed");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_skill_dir_at_root_and_nested() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();

        // Nested: root/pkg/my-skill/SKILL.md
        let nested = root.join("pkg").join("my-skill");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(nested.join("SKILL.md"), "---\nname: my-skill\n---\n").unwrap();

        let found = find_skill_src(root, "my-skill").unwrap();
        assert_eq!(found, nested);

        // Root itself is the skill dir.
        let direct = tempfile::tempdir().unwrap();
        std::fs::write(direct.path().join("SKILL.md"), "x").unwrap();
        let direct_named = direct.path().join("ignored");
        // file_name of direct.path() won't match "my-skill"; build a named dir.
        std::fs::create_dir_all(&direct_named).unwrap();
        std::fs::write(direct_named.join("SKILL.md"), "x").unwrap();
        assert_eq!(
            find_skill_src(&direct_named, "ignored").unwrap(),
            direct_named
        );
    }

    #[test]
    fn skill_finder_returns_none_for_wrong_name() {
        let tmp = tempfile::tempdir().unwrap();
        let d = tmp.path().join("other");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("SKILL.md"), "x").unwrap();
        assert!(find_skill_src(tmp.path(), "nope").is_none());
    }

    #[test]
    fn finds_agent_md_by_stem_with_frontmatter() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let agent = root.join("agents");
        std::fs::create_dir_all(&agent).unwrap();
        std::fs::write(
            agent.join("my-agent.md"),
            "---\nname: my-agent\ndescription: does things\n---\nbody\n",
        )
        .unwrap();
        // A SKILL.md must be ignored even if it shares a stem search.
        std::fs::write(root.join("SKILL.md"), "---\nname: x\n---\n").unwrap();

        let found = find_agent_src(root, "my-agent").unwrap();
        assert_eq!(found, agent.join("my-agent.md"));
        assert!(find_agent_src(root, "missing").is_none());
    }
}
