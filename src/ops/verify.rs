use console::style;

use crate::config;
use crate::error::Result;
use crate::inventory::{self, Config, LinkStrategy, TrackedKind, hash_resource};

#[derive(Debug)]
pub struct IntegrityIssue {
    pub name: String,
    pub kind: TrackedKind,
    pub expected: String,
    pub actual: String,
}

impl std::fmt::Display for IntegrityIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} {} has been modified outside agentspec (expected {}, got {})",
            self.kind, self.name, self.expected, self.actual
        )
    }
}

/// Check all managed resources for hash mismatches. Returns issues found.
pub fn verify_integrity() -> Result<Vec<IntegrityIssue>> {
    let lockfile = inventory::load_config()?;
    check_lockfile(&lockfile)
}

fn check_lockfile(lockfile: &Config) -> Result<Vec<IntegrityIssue>> {
    let base = config::agents_base_dir();
    let mut issues = Vec::new();

    for resource in &lockfile.resources {
        let abs_path = base.join(&resource.path);
        if !abs_path.exists() {
            issues.push(IntegrityIssue {
                name: resource.name.clone(),
                kind: resource.kind,
                expected: resource.hash.clone(),
                actual: "MISSING".to_string(),
            });
            continue;
        }

        let actual = hash_resource(resource.kind, &abs_path)?;
        if actual != resource.hash {
            issues.push(IntegrityIssue {
                name: resource.name.clone(),
                kind: resource.kind,
                expected: resource.hash.clone(),
                actual: actual.clone(),
            });
        }

        // For copy-strategy links, verify the copy matches the canonical version
        for link in &resource.links {
            if link.strategy != LinkStrategy::Copy {
                continue;
            }
            let link_path = std::path::Path::new(&link.path);
            if !link_path.exists() {
                continue;
            }
            let copy_hash = hash_resource(resource.kind, link_path)?;
            if copy_hash != resource.hash {
                issues.push(IntegrityIssue {
                    name: format!("{} (copy in {})", resource.name, link.tool),
                    kind: resource.kind,
                    expected: resource.hash.clone(),
                    actual: copy_hash,
                });
            }
        }
    }

    Ok(issues)
}

/// Print integrity warnings to stderr (for passive use by other commands).
pub fn warn_integrity_issues(issues: &[IntegrityIssue]) {
    for issue in issues {
        eprintln!("  {} {}", style("⚠").yellow().bold(), issue);
    }
}

/// Explicit `agentspec verify` command.
pub fn verify(accept: bool, accept_name: Option<&str>, json: bool) -> Result<()> {
    let mut lockfile = inventory::load_config()?;
    let base = config::agents_base_dir();

    if accept {
        // Re-hash and update lockfile
        let mut updated = 0;
        for resource in &mut lockfile.resources {
            if let Some(name) = accept_name
                && resource.name != name
            {
                continue;
            }
            let abs_path = base.join(&resource.path);
            if !abs_path.exists() {
                continue;
            }
            let new_hash = hash_resource(resource.kind, &abs_path)?;
            if new_hash != resource.hash {
                resource.hash = new_hash;
                resource.updated_at = chrono::Utc::now().to_rfc3339();
                updated += 1;
            }
        }
        inventory::save_config(&lockfile)?;
        if !json {
            println!(
                "  {} Updated {} resource hash(es)",
                style("✓").green().bold(),
                updated
            );
        }
        return Ok(());
    }

    let issues = check_lockfile(&lockfile)?;

    if json {
        let out: Vec<serde_json::Value> = issues
            .iter()
            .map(|i| {
                serde_json::json!({
                    "name": i.name,
                    "kind": format!("{}", i.kind),
                    "expected": i.expected,
                    "actual": i.actual,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else if issues.is_empty() {
        println!(
            "  {} All {} resources verified",
            style("✓").green().bold(),
            lockfile.resources.len()
        );
    } else {
        warn_integrity_issues(&issues);
        println!(
            "\n  {} {} integrity issue(s) found",
            style("✗").red().bold(),
            issues.len()
        );
    }

    if !issues.is_empty() {
        std::process::exit(1);
    }

    Ok(())
}
