//! `agentspec bootstrap` — teach the machine's AI tools how to use agentspec.
//!
//! agentspec is only useful if the agents on the machine know it exists. The
//! binary ships its own usage skills, and bootstrap writes them into the
//! canonical store, tracks them like any other managed resource, and links them
//! into every installed tool. It is idempotent: re-running refreshes bundled
//! content that agentspec itself owns, and `--force` re-links tool copies that
//! drifted.

use console::style;

use crate::config;
use crate::error::Result;
use crate::inventory::{Config, SourceType, TrackedKind, TrackedResource, hash_resource};
use crate::ir::ResourceKind;
use crate::ops::link;
use crate::tools;

/// Marks a resource as owned by the binary rather than by a git or local
/// source, so `bootstrap` may refresh it while leaving user resources alone.
pub const BUNDLED_SOURCE: &str = "bundled:agentspec";

/// Skills compiled into the binary, as (name, SKILL.md contents).
const BUNDLED_SKILLS: &[(&str, &str)] = &[
    (
        "agentspec-usage",
        include_str!("../../../../skills/agentspec-usage/SKILL.md"),
    ),
    (
        "resource-conventions",
        include_str!("../../../../skills/resource-conventions/SKILL.md"),
    ),
];

#[derive(Debug, serde::Serialize)]
pub struct BootstrapReport {
    pub skills: Vec<SkillOutcome>,
    pub tools: Vec<String>,
    pub store_dir: String,
}

#[derive(Debug, serde::Serialize)]
pub struct SkillOutcome {
    pub name: String,
    /// `installed`, `updated`, `unchanged`, or `skipped` (user-owned copy kept).
    pub status: &'static str,
    pub linked: Vec<String>,
}

/// Resolve which tools to link into: an explicit list, or every installed tool.
fn target_slugs(requested: Option<&[String]>) -> Vec<String> {
    match requested {
        Some(slugs) if !slugs.is_empty() => slugs.to_vec(),
        _ => tools::installed_tools()
            .iter()
            .map(|t| t.slug().to_string())
            .collect(),
    }
}

pub fn bootstrap(
    cfg: &mut Config,
    requested_tools: Option<&[String]>,
    force: bool,
    copy: bool,
    json: bool,
) -> Result<()> {
    let slugs = target_slugs(requested_tools);
    let mut outcomes = Vec::new();

    for (name, contents) in BUNDLED_SKILLS {
        let dir = config::shared_skills_dir().join(name);
        let skill_md = dir.join("SKILL.md");
        let tracked_is_bundled = cfg
            .find(name, TrackedKind::Skill)
            .is_some_and(|r| r.source == BUNDLED_SOURCE);

        // A same-named skill the user brought themselves is never overwritten;
        // adoption elsewhere in agentspec follows the same rule.
        let user_owned = skill_md.exists() && !tracked_is_bundled;
        if user_owned && !force {
            outcomes.push(SkillOutcome {
                name: (*name).to_string(),
                status: "skipped",
                linked: Vec::new(),
            });
            continue;
        }

        let current = std::fs::read_to_string(&skill_md).ok();
        let status = match current.as_deref() {
            Some(existing) if existing == *contents => "unchanged",
            Some(_) => "updated",
            None => "installed",
        };
        if status != "unchanged" {
            std::fs::create_dir_all(&dir)?;
            std::fs::write(&skill_md, contents)?;
        }

        let hash = hash_resource(TrackedKind::Skill, &dir)?;
        match cfg.find_mut(name, TrackedKind::Skill) {
            Some(existing) => {
                existing.hash = hash;
                existing.source = BUNDLED_SOURCE.to_string();
                existing.source_type = SourceType::Local;
                existing.updated_at = chrono::Utc::now().to_rfc3339();
            }
            None => cfg.add(TrackedResource::new(
                (*name).to_string(),
                TrackedKind::Skill,
                BUNDLED_SOURCE.to_string(),
                SourceType::Local,
                format!("skills/{name}"),
                hash,
            )),
        }

        // Relink to pick up refreshed content, and to repair a tool copy that
        // was removed or predates this install.
        let mut linked = Vec::new();
        for slug in &slugs {
            if force || status != "unchanged" {
                let _ = link::unlink(cfg, ResourceKind::Skill, name, slug);
            }
            match link::link(cfg, ResourceKind::Skill, name, slug, copy) {
                Ok(()) => linked.push(slug.clone()),
                // Already linked is the steady state, not an error.
                Err(crate::error::AppError::AlreadyExists(_)) => linked.push(slug.clone()),
                Err(e) => eprintln!("  {} {slug}: {e}", style("~").yellow()),
            }
        }

        outcomes.push(SkillOutcome {
            name: (*name).to_string(),
            status,
            linked,
        });
    }

    crate::inventory::save_config(cfg)?;

    let report = BootstrapReport {
        skills: outcomes,
        tools: slugs,
        store_dir: config::shared_skills_dir().to_string_lossy().to_string(),
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }

    println!("{}", style("agentspec bootstrap").bold());
    for s in &report.skills {
        let mark = match s.status {
            "skipped" => style("~").yellow(),
            "unchanged" => style("=").dim(),
            _ => style("✓").green(),
        };
        println!(
            "  {} {:<22} {:<10} → {}",
            mark,
            s.name,
            s.status,
            if s.linked.is_empty() {
                "(no tools)".to_string()
            } else {
                s.linked.join(", ")
            }
        );
    }
    let live: Vec<&str> = report
        .skills
        .iter()
        .filter(|s| !s.linked.is_empty())
        .map(|s| s.name.as_str())
        .collect();
    if report.tools.is_empty() {
        println!(
            "  {} no installed AI tools detected; skills are in {}",
            style("~").yellow(),
            report.store_dir
        );
    } else if live.is_empty() {
        println!(
            "\n  {} nothing was linked; skills are in {}",
            style("~").yellow(),
            report.store_dir
        );
    } else {
        println!(
            "\n  Agents in {} tool(s) can now read: {}",
            report.tools.len(),
            live.join(", ")
        );
    }
    if report.skills.iter().any(|s| s.status == "skipped") {
        println!(
            "  {} a same-named skill you own was kept; pass --force to replace it",
            style("~").yellow()
        );
    }
    Ok(())
}
