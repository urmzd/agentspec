use std::path::{Path, PathBuf};

use console::style;

use crate::error::Result;
use crate::ops::{discover, link, verify};

/// Run the full sync pipeline: discover → adopt → link → verify.
pub fn sync(
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
    discover::refresh_cache_with_root(broad_root, extra_paths)?;

    let cfg = crate::inventory::load_config()?;
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
        discover::adopt_all(false, false)?;
    }

    // 3. Ensure all managed resources are linked to all installed tools
    if !json {
        println!("  {} Ensuring links...", style("→").cyan());
    }
    let linked = link::ensure_all_links(false)?;
    if !json && linked > 0 {
        println!(
            "  {} Created {} new link(s)",
            style("✓").green().bold(),
            linked
        );
    }

    // 4. Verify integrity
    let issues = verify::verify_integrity()?;

    // 5. Report
    let cfg = crate::inventory::load_config()?;

    if json {
        let report = serde_json::json!({
            "managed": cfg.resources.len(),
            "discovered": cfg.discovered.len(),
            "links_created": linked,
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
