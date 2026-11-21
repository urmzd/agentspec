//! Apply the pending findings from [`verify::inspect`].
//!
//! `sync` calls [`reconcile`] so that ordinary change never shows up as an
//! integrity issue: recorded hashes follow observed files and store updates,
//! records for observed files that are gone are dropped, and untouched tool
//! copies are rewritten when the store moves on. Anything a person edited is
//! left exactly as it is and reported by `verify`.

use std::path::{Path, PathBuf};

use crate::config;
use crate::error::Result;
use crate::inventory::{Config, LinkStrategy, TrackedKind};
use crate::ops::refresh;
use crate::ops::verify::{self, Finding};

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Reconciled {
    /// Recorded hashes updated to follow new content.
    pub records_updated: usize,
    /// Records dropped because the observed file no longer exists.
    pub records_dropped: usize,
    /// Stale tool copies rewritten from the store.
    pub copies_refreshed: usize,
}

pub fn reconcile(cfg: &mut Config) -> Reconciled {
    reconcile_at(cfg, &config::agents_base_dir())
}

pub(crate) fn reconcile_at(cfg: &mut Config, base: &Path) -> Reconciled {
    let mut out = Reconciled::default();
    let mut gone: Vec<(String, TrackedKind)> = Vec::new();

    for resource in &mut cfg.resources {
        // One unreadable resource must not stop the rest from reconciling.
        let findings = match verify::inspect(resource, base) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("  warning: could not inspect {}: {e}", resource.name);
                continue;
            }
        };
        let store_path = base.join(&resource.path);
        let kind = resource.kind;

        for finding in findings {
            match finding {
                Finding::Gone => gone.push((resource.name.clone(), kind)),
                Finding::Changed { actual } => {
                    resource.hash = actual;
                    resource.updated_at = chrono::Utc::now().to_rfc3339();
                    out.records_updated += 1;
                }
                Finding::CopyUnrecorded { tool, hash } => {
                    if let Some(link) = resource.links.iter_mut().find(|l| l.tool == tool) {
                        link.hash = Some(hash);
                    }
                }
                Finding::CopyStale { tool } => {
                    let Some(link) = resource.links.iter_mut().find(|l| l.tool == tool) else {
                        continue;
                    };
                    match refresh::write_copy(kind, &store_path, link) {
                        Ok(()) => out.copies_refreshed += 1,
                        Err(e) => eprintln!(
                            "  warning: could not refresh {} in {tool}: {e}",
                            resource.name
                        ),
                    }
                }
                // Pulled in by the refresh step; issues are left for a person.
                Finding::OriginAhead { .. }
                | Finding::Missing
                | Finding::Modified { .. }
                | Finding::CopyEdited { .. } => {}
            }
        }
    }

    for (name, kind) in gone {
        cfg.remove(&name, kind);
        out.records_dropped += 1;
    }
    out
}

#[derive(Debug, Default)]
pub struct Repaired {
    pub count: usize,
    /// Where the replaced copies were kept, when any were replaced.
    pub backup_dir: Option<String>,
}

/// Rewrite every copy that was edited in place from the store, keeping the
/// replaced version in a timestamped backup directory first.
pub fn repair_edited_copies(cfg: &mut Config, only: Option<&str>) -> Result<Repaired> {
    let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
    let backup_root = config::config_dir().join("backups").join(stamp);
    repair_at(cfg, only, &config::agents_base_dir(), &backup_root)
}

pub(crate) fn repair_at(
    cfg: &mut Config,
    only: Option<&str>,
    base: &Path,
    backup_root: &Path,
) -> Result<Repaired> {
    let mut out = Repaired::default();

    for resource in &mut cfg.resources {
        if only.is_some_and(|name| resource.name != name) {
            continue;
        }
        let store_path = base.join(&resource.path);
        let kind = resource.kind;
        for finding in verify::inspect(resource, base)? {
            let Finding::CopyEdited { tool, .. } = finding else {
                continue;
            };
            let Some(link) = resource
                .links
                .iter_mut()
                .find(|l| l.tool == tool && l.strategy == LinkStrategy::Copy)
            else {
                continue;
            };
            let link_path = PathBuf::from(&link.path);
            let file_name = link_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| resource.name.clone());
            let backup = backup_root.join(&tool).join(file_name);
            if let Some(parent) = backup.parent() {
                std::fs::create_dir_all(parent)?;
            }
            // Keep the replaced version before anything is overwritten.
            if link_path.is_dir() {
                crate::ops::manage::copy_dir(&link_path, &backup)?;
            } else {
                std::fs::copy(&link_path, &backup)?;
            }
            refresh::write_copy(kind, &store_path, link)?;
            out.count += 1;
        }
    }

    if out.count > 0 {
        out.backup_dir = Some(backup_root.to_string_lossy().to_string());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{ResourceLink, SourceType, TrackedResource, hash_resource};

    fn write_skill(dir: &Path, body: &str) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(
            dir.join("SKILL.md"),
            format!("---\nname: demo\ndescription: d\n---\n{body}\n"),
        )
        .unwrap();
    }

    fn read_skill(dir: &Path) -> String {
        std::fs::read_to_string(dir.join("SKILL.md")).unwrap()
    }

    /// A discovered skill at v1 with one copy link holding `copy_body`.
    fn setup(base: &Path, copy: &Path, copy_body: &str, recorded: bool) -> Config {
        let store = base.join("skills/demo");
        write_skill(&store, "v1");
        let hash = hash_resource(TrackedKind::Skill, &store).unwrap();
        let mut resource = TrackedResource::new(
            "demo".into(),
            TrackedKind::Skill,
            "discovered".into(),
            SourceType::Discovered,
            "skills/demo".into(),
            hash.clone(),
        );
        write_skill(copy, copy_body);
        resource.links.push(ResourceLink {
            tool: "opencode".into(),
            strategy: LinkStrategy::Copy,
            path: copy.to_string_lossy().to_string(),
            hash: recorded.then_some(hash),
        });
        let mut cfg = Config::empty();
        cfg.add(resource);
        cfg
    }

    #[test]
    fn a_store_update_reaches_untouched_copies_and_leaves_no_issue() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path().join("agents");
        let copy = tmp.path().join("tool/demo");
        let mut cfg = setup(&base, &copy, "v1", true);

        // Something outside agentspec (a dotfile manager) updates the store.
        write_skill(&base.join("skills/demo"), "v2");

        let out = reconcile_at(&mut cfg, &base);
        assert_eq!(out.records_updated, 1);
        assert_eq!(out.copies_refreshed, 1);
        assert!(read_skill(&copy).contains("v2"));

        let check = verify::check_at(&cfg, &base).unwrap();
        assert!(check.issues.is_empty());
        assert_eq!(check.pending, 0);
        // A second run has nothing left to do.
        assert_eq!(reconcile_at(&mut cfg, &base), Reconciled::default());
    }

    #[test]
    fn a_copy_edited_in_place_is_never_overwritten_by_sync() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path().join("agents");
        let copy = tmp.path().join("tool/demo");
        let mut cfg = setup(&base, &copy, "my local edit", true);
        write_skill(&base.join("skills/demo"), "v2");

        let out = reconcile_at(&mut cfg, &base);
        assert_eq!(out.copies_refreshed, 0);
        assert!(read_skill(&copy).contains("my local edit"));
        assert_eq!(verify::check_at(&cfg, &base).unwrap().issues.len(), 1);
    }

    #[test]
    fn repair_keeps_the_edited_copy_before_rewriting_it() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path().join("agents");
        let copy = tmp.path().join("tool/demo");
        let backups = tmp.path().join("backups");
        let mut cfg = setup(&base, &copy, "my local edit", true);

        let out = repair_at(&mut cfg, None, &base, &backups).unwrap();
        assert_eq!(out.count, 1);
        assert!(read_skill(&copy).contains("v1"));
        assert!(read_skill(&backups.join("opencode/demo")).contains("my local edit"));
        assert!(verify::check_at(&cfg, &base).unwrap().issues.is_empty());
    }

    #[test]
    fn repair_honors_the_name_filter() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path().join("agents");
        let copy = tmp.path().join("tool/demo");
        let mut cfg = setup(&base, &copy, "my local edit", true);

        let out = repair_at(&mut cfg, Some("other"), &base, &tmp.path().join("b")).unwrap();
        assert_eq!(out.count, 0);
        assert!(out.backup_dir.is_none());
        assert!(read_skill(&copy).contains("my local edit"));
    }

    #[test]
    fn a_matching_copy_from_an_older_version_gets_its_hash_recorded() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path().join("agents");
        let copy = tmp.path().join("tool/demo");
        let mut cfg = setup(&base, &copy, "v1", false);

        reconcile_at(&mut cfg, &base);
        let link = &cfg.resources[0].links[0];
        assert_eq!(link.hash.as_deref(), Some(cfg.resources[0].hash.as_str()));
    }

    #[test]
    fn records_for_observed_files_follow_edits_and_drop_when_gone() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("project/llms.txt");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, "one").unwrap();
        let mut cfg = Config::empty();
        cfg.add(TrackedResource::new(
            "project/llms.txt".into(),
            TrackedKind::LlmsTxt,
            "discovered".into(),
            SourceType::Discovered,
            file.to_string_lossy().to_string(),
            hash_resource(TrackedKind::LlmsTxt, &file).unwrap(),
        ));

        std::fs::write(&file, "two").unwrap();
        assert_eq!(reconcile_at(&mut cfg, tmp.path()).records_updated, 1);
        assert!(
            verify::check_at(&cfg, tmp.path())
                .unwrap()
                .issues
                .is_empty()
        );

        std::fs::remove_file(&file).unwrap();
        assert_eq!(reconcile_at(&mut cfg, tmp.path()).records_dropped, 1);
        assert!(cfg.resources.is_empty());
    }
}
