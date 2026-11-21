//! Integrity checking.
//!
//! A recorded hash means different things depending on who owns the content:
//!
//! - **Observed files** (project configs, instruction files, `llms.txt`,
//!   memories, plans) are adopted in place. Their owner edits them as part of
//!   normal work, so a new hash is a change to follow, never tampering.
//! - **Store-owned resources with no upstream** (`discovered` skills and
//!   agents) have the store as their source of truth. Whatever writes the
//!   store (a dotfile manager, an installer) is updating the resource.
//! - **Store-owned resources with an upstream** (`git`, `local`) should match
//!   what the last refresh wrote. A different hash there is real drift.
//! - **Tool copies** are derived from the store. A copy that still matches
//!   what agentspec last wrote is merely stale when the store moves on. A copy
//!   that matches neither was edited in place.
//!
//! [`inspect`] classifies one resource into [`Finding`]s. Pending findings are
//! applied by `sync` (see `reconcile`); issues need a person and are the only
//! thing reported as an integrity issue.

use std::path::Path;

use console::style;

use crate::config;
use crate::error::Result;
use crate::inventory::{
    Config, LinkStrategy, SourceType, TrackedKind, TrackedResource, hash_resource,
};

/// One observation about a tracked resource.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Finding {
    // -- pending: `sync` reconciles these on its own --------------------------
    /// An observed file no longer exists. The record is dropped.
    Gone,
    /// An observed file, or a store-owned resource with no upstream, has new
    /// content. The recorded hash follows it.
    Changed { actual: String },
    /// A copy is behind the store and was not touched since agentspec wrote
    /// it. The copy is rewritten.
    CopyStale { tool: String },
    /// A copy matches the store but its hash was never recorded.
    CopyUnrecorded { tool: String, hash: String },
    /// The tool-directory origin of a `local` resource is ahead of the store.
    /// The refresh step of `sync` pulls it in.
    OriginAhead { tool: String },

    // -- issues: need a person ----------------------------------------------
    /// A store-owned resource is missing from the store.
    Missing,
    /// A store-owned resource with an upstream differs from what was recorded.
    Modified { actual: String },
    /// A copy differs from the store and from what agentspec last wrote.
    CopyEdited {
        tool: String,
        expected: String,
        actual: String,
    },
}

#[cfg(test)]
impl Finding {
    pub fn is_issue(&self) -> bool {
        matches!(
            self,
            Finding::Missing | Finding::Modified { .. } | Finding::CopyEdited { .. }
        )
    }
}

/// True for resources adopted in place: agentspec records them but does not
/// own the file.
pub fn is_observed(resource: &TrackedResource) -> bool {
    !matches!(resource.kind, TrackedKind::Skill | TrackedKind::Agent)
        || Path::new(&resource.path).is_absolute()
}

/// Classify one resource against the content on disk. `base` is the shared
/// store root (`~/.agents`).
pub fn inspect(resource: &TrackedResource, base: &Path) -> Result<Vec<Finding>> {
    let abs_path = base.join(&resource.path);
    let observed = is_observed(resource);

    if !abs_path.exists() {
        return Ok(vec![if observed {
            Finding::Gone
        } else {
            Finding::Missing
        }]);
    }

    let mut findings = Vec::new();
    let actual = hash_resource(resource.kind, &abs_path)?;
    // The hash a skill copy is compared against: the store as it is now.
    let mut store_hash = resource.hash.clone();

    if actual != resource.hash {
        if observed || resource.source_type == SourceType::Discovered {
            store_hash = actual.clone();
            findings.push(Finding::Changed { actual });
        } else {
            // The store itself is in question, so its copies cannot be judged.
            findings.push(Finding::Modified { actual });
            return Ok(findings);
        }
    }

    for link in &resource.links {
        if link.strategy != LinkStrategy::Copy {
            continue;
        }
        let link_path = Path::new(&link.path);
        if !link_path.exists() {
            continue;
        }
        let copy_hash = hash_resource(resource.kind, link_path)?;
        let expected = if resource.kind == TrackedKind::Agent {
            super::agent_render::expected_hash(&abs_path, &link.tool)?
        } else {
            store_hash.clone()
        };

        if copy_hash == expected {
            if link.hash.as_deref() != Some(copy_hash.as_str()) {
                findings.push(Finding::CopyUnrecorded {
                    tool: link.tool.clone(),
                    hash: copy_hash,
                });
            }
            continue;
        }

        if resource.source_type == SourceType::Local && link.path == resource.source {
            findings.push(Finding::OriginAhead {
                tool: link.tool.clone(),
            });
            continue;
        }

        // Untouched since agentspec wrote it? Links from older versions carry
        // no hash; for a skill the previously recorded store hash stands in.
        let untouched = match &link.hash {
            Some(written) => *written == copy_hash,
            None => resource.kind == TrackedKind::Skill && copy_hash == resource.hash,
        };
        findings.push(if untouched {
            Finding::CopyStale {
                tool: link.tool.clone(),
            }
        } else {
            Finding::CopyEdited {
                tool: link.tool.clone(),
                expected,
                actual: copy_hash,
            }
        });
    }

    Ok(findings)
}

#[derive(Debug)]
pub struct IntegrityIssue {
    pub name: String,
    pub kind: TrackedKind,
    pub expected: String,
    pub actual: String,
    /// What is wrong, phrased to follow "<kind> <name> ".
    what: &'static str,
}

impl std::fmt::Display for IntegrityIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} {} {} (expected {}, got {})",
            self.kind, self.name, self.what, self.expected, self.actual
        )
    }
}

/// The result of checking every tracked resource.
#[derive(Debug, Default)]
pub struct Check {
    pub issues: Vec<IntegrityIssue>,
    /// Changes `sync` will reconcile by itself. Not integrity issues.
    pub pending: usize,
}

/// Check all tracked resources. Read-only.
pub fn check(cfg: &Config) -> Result<Check> {
    check_at(cfg, &config::agents_base_dir())
}

pub(crate) fn check_at(cfg: &Config, base: &Path) -> Result<Check> {
    let mut check = Check::default();
    for resource in &cfg.resources {
        for finding in inspect(resource, base)? {
            match finding {
                Finding::Missing => check.issues.push(IntegrityIssue {
                    name: resource.name.clone(),
                    kind: resource.kind,
                    expected: resource.hash.clone(),
                    actual: "MISSING".to_string(),
                    what: "is missing from the store",
                }),
                Finding::Modified { actual } => check.issues.push(IntegrityIssue {
                    name: resource.name.clone(),
                    kind: resource.kind,
                    expected: resource.hash.clone(),
                    actual,
                    what: "has been modified outside agentspec",
                }),
                Finding::CopyEdited {
                    tool,
                    expected,
                    actual,
                } => check.issues.push(IntegrityIssue {
                    name: format!("{} (copy in {tool})", resource.name),
                    kind: resource.kind,
                    expected,
                    actual,
                    what: "differs from the store and from what agentspec wrote",
                }),
                // Recording a baseline hash is bookkeeping, not a change.
                Finding::CopyUnrecorded { .. } => {}
                Finding::Gone
                | Finding::Changed { .. }
                | Finding::CopyStale { .. }
                | Finding::OriginAhead { .. } => check.pending += 1,
            }
        }
    }
    Ok(check)
}

/// Integrity issues only. Used after `sync` has reconciled what it can.
pub fn verify_integrity(cfg: &Config) -> Result<Vec<IntegrityIssue>> {
    Ok(check(cfg)?.issues)
}

/// Print integrity warnings to stderr (for passive use by other commands).
pub fn warn_integrity_issues(issues: &[IntegrityIssue]) {
    for issue in issues {
        eprintln!("  {} {}", style("⚠").yellow().bold(), issue);
    }
}

/// Explicit `agentspec manage verify` command. Returns the number of integrity
/// issues found; the caller maps a nonzero count to a failing exit code
/// after the config has been saved.
pub fn verify(
    cfg: &mut Config,
    accept: bool,
    repair: bool,
    accept_name: Option<&str>,
    json: bool,
) -> Result<usize> {
    let base = config::agents_base_dir();

    if repair {
        let repaired = super::reconcile::repair_edited_copies(cfg, accept_name)?;
        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "repaired": repaired.count,
                    "backup": repaired.backup_dir,
                }))?
            );
        } else if repaired.count == 0 {
            println!("  {} No edited copies to repair", style("✓").green().bold());
        } else {
            println!(
                "  {} Rewrote {} copy(ies) from the store",
                style("✓").green().bold(),
                repaired.count
            );
            if let Some(dir) = &repaired.backup_dir {
                println!("  {} Previous versions kept in {dir}", style("→").dim());
            }
        }
        return Ok(0);
    }

    if accept {
        // Re-hash and update lockfile
        let mut updated = 0;
        for resource in &mut cfg.resources {
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
        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({ "accepted": updated }))?
            );
        } else {
            println!(
                "  {} Updated {} resource hash(es)",
                style("✓").green().bold(),
                updated
            );
        }
        return Ok(0);
    }

    let Check { issues, pending } = check(cfg)?;

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
    } else {
        if issues.is_empty() {
            println!(
                "  {} All {} resources verified",
                style("✓").green().bold(),
                cfg.resources.len()
            );
        } else {
            warn_integrity_issues(&issues);
            println!(
                "\n  {} {} integrity issue(s) found",
                style("✗").red().bold(),
                issues.len()
            );
            if issues.iter().any(|i| i.name.contains("(copy in ")) {
                println!(
                    "  {} An edited copy: move your edit into the store, or run \
                     `agentspec manage verify --repair` to rewrite it from the store",
                    style("→").dim()
                );
            }
        }
        if pending > 0 {
            println!(
                "  {} {pending} change(s) pending. Run `agentspec sync` to apply them",
                style("→").dim()
            );
        }
    }

    Ok(issues.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::ResourceLink;

    fn write_skill(dir: &Path, body: &str) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(
            dir.join("SKILL.md"),
            format!("---\nname: demo\ndescription: d\n---\n{body}\n"),
        )
        .unwrap();
    }

    fn skill(base: &Path, source_type: SourceType) -> TrackedResource {
        let store = base.join("skills/demo");
        write_skill(&store, "v1");
        TrackedResource::new(
            "demo".into(),
            TrackedKind::Skill,
            "discovered".into(),
            source_type,
            "skills/demo".into(),
            hash_resource(TrackedKind::Skill, &store).unwrap(),
        )
    }

    fn copy_link(dir: &Path, body: &str, hash: Option<String>) -> ResourceLink {
        write_skill(dir, body);
        ResourceLink {
            tool: "opencode".into(),
            strategy: LinkStrategy::Copy,
            path: dir.to_string_lossy().to_string(),
            hash,
            suspended: false,
        }
    }

    #[test]
    fn a_store_update_with_no_upstream_is_a_change_not_an_issue() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();
        let resource = skill(base, SourceType::Discovered);
        write_skill(&base.join("skills/demo"), "v2");

        let findings = inspect(&resource, base).unwrap();
        assert!(matches!(findings.as_slice(), [Finding::Changed { .. }]));
        assert!(!findings[0].is_issue());
    }

    #[test]
    fn a_store_edit_under_an_upstream_is_an_issue() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();
        let resource = skill(base, SourceType::Git);
        write_skill(&base.join("skills/demo"), "v2");

        let findings = inspect(&resource, base).unwrap();
        assert!(matches!(findings.as_slice(), [Finding::Modified { .. }]));
    }

    #[test]
    fn an_untouched_copy_behind_the_store_is_stale() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();
        let mut resource = skill(base, SourceType::Discovered);
        // The copy holds v1, which is what agentspec wrote.
        let copy = tmp.path().join("tool/demo");
        let written = resource.hash.clone();
        resource.links.push(copy_link(&copy, "v1", Some(written)));
        write_skill(&base.join("skills/demo"), "v2");

        let findings = inspect(&resource, base).unwrap();
        assert!(findings.contains(&Finding::CopyStale {
            tool: "opencode".into()
        }));
        assert!(findings.iter().all(|f| !f.is_issue()));
    }

    #[test]
    fn a_copy_from_an_older_version_falls_back_to_the_recorded_store_hash() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();
        let mut resource = skill(base, SourceType::Discovered);
        resource
            .links
            .push(copy_link(&tmp.path().join("tool/demo"), "v1", None));
        write_skill(&base.join("skills/demo"), "v2");

        let findings = inspect(&resource, base).unwrap();
        assert!(findings.contains(&Finding::CopyStale {
            tool: "opencode".into()
        }));
    }

    #[test]
    fn a_copy_edited_in_place_is_an_issue() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();
        let mut resource = skill(base, SourceType::Discovered);
        let written = resource.hash.clone();
        resource.links.push(copy_link(
            &tmp.path().join("tool/demo"),
            "edited by hand",
            Some(written),
        ));

        let findings = inspect(&resource, base).unwrap();
        assert!(matches!(findings.as_slice(), [Finding::CopyEdited { .. }]));
    }

    #[test]
    fn an_observed_file_is_followed_and_dropped_when_gone() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("project/AGENTS.md");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, "one").unwrap();
        let resource = TrackedResource::new(
            "project/AGENTS.md".into(),
            TrackedKind::ProjectConfig,
            "discovered".into(),
            SourceType::Discovered,
            file.to_string_lossy().to_string(),
            hash_resource(TrackedKind::ProjectConfig, &file).unwrap(),
        );
        assert!(is_observed(&resource));
        assert!(inspect(&resource, tmp.path()).unwrap().is_empty());

        std::fs::write(&file, "two").unwrap();
        assert!(matches!(
            inspect(&resource, tmp.path()).unwrap().as_slice(),
            [Finding::Changed { .. }]
        ));

        std::fs::remove_file(&file).unwrap();
        assert_eq!(inspect(&resource, tmp.path()).unwrap(), vec![Finding::Gone]);
    }

    #[test]
    fn a_missing_store_resource_is_an_issue() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();
        let resource = skill(base, SourceType::Discovered);
        std::fs::remove_dir_all(base.join("skills/demo")).unwrap();
        assert_eq!(inspect(&resource, base).unwrap(), vec![Finding::Missing]);
    }
}
