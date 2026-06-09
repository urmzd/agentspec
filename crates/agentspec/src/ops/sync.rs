use std::path::{Path, PathBuf};

use console::style;

use crate::error::Result;
use crate::inventory::Config;
use crate::mcp;
use crate::ops::{discover, link, project_sync, verify};

/// Run the full sync pipeline: discover → adopt → link → verify.
pub fn sync(
    cfg: &mut Config,
    root: Option<&Path>,
    fast: bool,
    auto_adopt: bool,
    json: bool,
    extra_paths: &[PathBuf],
) -> Result<()> {
    // 1. Discover
    if !json {
        println!("  {} Scanning for resources...", style("→").cyan());
    }

    let broad_root = if fast { None } else { root };
    discover::refresh_cache_with_root(cfg, broad_root, extra_paths)?;

    let discovered_count = cfg.discovered.len();

    if !json && discovered_count > 0 {
        println!(
            "  {} Found {} unmanaged resource(s)",
            style("○").dim(),
            discovered_count
        );
    }

    // 2. Adopt
    if auto_adopt && discovered_count > 0 {
        if !json {
            println!("  {} Adopting resources...", style("→").cyan());
        }
        discover::adopt_all(cfg, false, false)?;
    }

    // 3. Ensure all managed resources are linked to all installed tools
    if !json {
        println!("  {} Ensuring links...", style("→").cyan());
    }
    let (reconciled, linked) = link::ensure_all_links(cfg, false)?;
    if !json && reconciled > 0 {
        println!(
            "  {} Reconciled {} existing link(s) into tracking",
            style("~").yellow(),
            reconciled
        );
    }
    if !json && linked > 0 {
        println!(
            "  {} Created {} new link(s)",
            style("✓").green().bold(),
            linked
        );
    }

    // 4. Resync tracked projects
    if !json {
        println!("  {} Resyncing tracked projects...", style("→").cyan());
    }
    let (projects_resynced, files_updated) = project_sync::resync_all(cfg, json).unwrap_or((0, 0));

    // 5. Discover and register MCP servers from .mcp.json files
    if !json {
        println!("  {} Discovering MCP servers...", style("→").cyan());
    }
    let project_roots = mcp::collect_project_roots();
    let _ = mcp::discover_and_register(&project_roots);

    // 6. Verify integrity
    let issues = verify::verify_integrity(cfg)?;

    // 7. Report
    if json {
        let report = serde_json::json!({
            "managed": cfg.resources.len(),
            "discovered": cfg.discovered.len(),
            "links_reconciled": reconciled,
            "links_created": linked,
            "projects_resynced": projects_resynced,
            "files_updated": files_updated,
            "integrity_issues": issues.len(),
        });
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        if !issues.is_empty() {
            verify::warn_integrity_issues(&issues);
        }

        let managed = cfg.resources.len();
        let remaining = cfg.discovered.len();

        println!();
        println!(
            "  {} {} managed, {} unmanaged, {} integrity issue(s)",
            if issues.is_empty() {
                style("✓").green().bold()
            } else {
                style("⚠").yellow().bold()
            },
            managed,
            remaining,
            issues.len()
        );

        if remaining > 0 {
            println!(
                "  {} Run with --adopt to consolidate unmanaged resources",
                style("→").dim()
            );
        }
    }

    Ok(())
}
