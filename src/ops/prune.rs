use console::style;

use crate::config;
use crate::error::Result;
use crate::inventory;
use crate::tools;

#[derive(Default)]
struct PruneReport {
    broken_resources: Vec<(String, String)>, // (name, kind)
    broken_symlinks: Vec<String>,            // tool link paths
    stale_discovered: Vec<String>,           // discovered names
    missing_projects: Vec<String>,           // project names
}

pub fn prune(dry_run: bool, json: bool) -> Result<()> {
    let mut cfg = inventory::load_config()?;
    let mut report = PruneReport::default();

    // 1. Broken managed resources (file/dir missing from ~/.agents/)
    for resource in &cfg.resources {
        let abs_path = config::agents_base_dir().join(&resource.path);
        if !abs_path.exists() {
            report
                .broken_resources
                .push((resource.name.clone(), resource.kind.to_string()));
        }
    }

    // 2. Broken symlinks in all tool dirs
    for tool in tools::installed_tools() {
        for dir_fn in [tool.skills_dir(), tool.agents_dir()].into_iter().flatten() {
            let Ok(entries) = std::fs::read_dir(&dir_fn) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_symlink() && !path.exists() {
                    report.broken_symlinks.push(path.display().to_string());
                }
            }
        }
    }

    // 3. Stale discovery cache entries (all locations gone)
    for disc in &cfg.discovered {
        let all_gone = disc
            .found_in
            .iter()
            .all(|loc| !std::path::Path::new(&loc.path).exists());
        if all_gone {
            report.stale_discovered.push(disc.name.clone());
        }
    }

    // 4. Project entries whose synced dir no longer exists
    for project in &cfg.projects {
        let synced_dir = config::shared_project_dir(&project.name);
        if !synced_dir.exists() {
            report.missing_projects.push(project.name.clone());
        }
    }

    let total = report.broken_resources.len()
        + report.broken_symlinks.len()
        + report.stale_discovered.len()
        + report.missing_projects.len();

    if json {
        let out = serde_json::json!({
            "dry_run": dry_run,
            "broken_resources": report.broken_resources.iter().map(|(n, k)| serde_json::json!({"name": n, "kind": k})).collect::<Vec<_>>(),
            "broken_symlinks": report.broken_symlinks,
            "stale_discovered": report.stale_discovered,
            "missing_projects": report.missing_projects,
            "total": total,
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
        if !dry_run {
            apply(&mut cfg, &report)?;
        }
        return Ok(());
    }

    if total == 0 {
        println!("  {} Nothing to prune", style("✓").green().bold());
        return Ok(());
    }

    // Print what was found
    for (name, kind) in &report.broken_resources {
        let label = if dry_run { "would remove" } else { "removing" };
        println!(
            "  {} {} {kind} '{name}' (missing from store)",
            style("✗").red().bold(),
            label,
        );
    }
    for path in &report.broken_symlinks {
        let label = if dry_run { "would remove" } else { "removing" };
        println!(
            "  {} {} broken symlink {}",
            style("✗").red().bold(),
            label,
            path,
        );
    }
    for name in &report.stale_discovered {
        let label = if dry_run { "would drop" } else { "dropping" };
        println!(
            "  {} {} stale discovered entry '{name}'",
            style("~").yellow().bold(),
            label,
        );
    }
    for name in &report.missing_projects {
        let label = if dry_run { "would remove" } else { "removing" };
        println!(
            "  {} {} project '{name}' (synced dir gone)",
            style("✗").red().bold(),
            label,
        );
    }

    if dry_run {
        println!(
            "\n  {} dry run — {} item(s) found. Re-run with --yes to apply.",
            style("→").cyan().bold(),
            total,
        );
    } else {
        apply(&mut cfg, &report)?;
        println!("\n  {} Pruned {} item(s)", style("✓").green().bold(), total,);
    }

    Ok(())
}

fn apply(cfg: &mut inventory::Config, report: &PruneReport) -> Result<()> {
    // Remove broken managed resources from config (no unlink needed — links are
    // already broken; the symlink scan above handles those separately)
    for (name, kind_str) in &report.broken_resources {
        let kind = parse_tracked_kind(kind_str);
        cfg.remove(name, kind);
    }

    // Delete broken symlinks from disk
    for path in &report.broken_symlinks {
        let _ = std::fs::remove_file(path);
    }

    // Drop stale discovery entries
    let stale: std::collections::HashSet<&str> =
        report.stale_discovered.iter().map(|s| s.as_str()).collect();
    cfg.discovered.retain(|d| !stale.contains(d.name.as_str()));

    // Remove missing project entries from config
    for name in &report.missing_projects {
        cfg.remove_project(name);
    }

    inventory::save_config(cfg)?;
    Ok(())
}

fn parse_tracked_kind(s: &str) -> inventory::TrackedKind {
    match s {
        "skill" => inventory::TrackedKind::Skill,
        "agent" => inventory::TrackedKind::Agent,
        "project-config" => inventory::TrackedKind::ProjectConfig,
        "instruction-file" => inventory::TrackedKind::InstructionFile,
        "llms-txt" => inventory::TrackedKind::LlmsTxt,
        "memory" => inventory::TrackedKind::Memory,
        "session" => inventory::TrackedKind::Session,
        "plan" => inventory::TrackedKind::Plan,
        _ => inventory::TrackedKind::Skill,
    }
}
