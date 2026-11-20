//! Explicit moves of skills and agents between scopes.
//!
//! Discovery never crosses a project boundary on its own (see `discover`), so
//! moving a resource between the global store and a project, or from one
//! project to another, is always a deliberate step:
//!
//! - promote: project -> global is `manage add <path>`.
//! - transfer: store, project or tool -> project is `manage transfer`, which
//!   copies the resource into the target project's own tool directories. With
//!   `--from <tool>` the source is a resource in one tool's project directory,
//!   so a project can move a skill from Claude Code to Codex in place. With
//!   `--move`, the source is removed afterwards: a managed source leaves the
//!   global store (demotion), a `--from` source leaves that tool's directory.

use std::path::{Path, PathBuf};

use crate::config;
use crate::error::{AppError, Result};
use crate::inventory::{Config, TrackedKind};
use crate::ops::{manage, remove};
use crate::tools;

/// A resolved transfer source.
#[derive(Debug, PartialEq, Eq)]
struct Source {
    name: String,
    kind: TrackedKind,
    path: PathBuf,
    managed: bool,
    /// Resolved from a tool's project directory via `--from`.
    from_tool: bool,
}

/// Resolve `source` as a path to a skill dir or agent `.md`, else as the name
/// of a managed skill or agent in the global store.
fn resolve_source(cfg: &Config, source: &str) -> Result<Source> {
    let as_path = PathBuf::from(source);
    if as_path.exists() {
        let path = std::fs::canonicalize(&as_path)?;
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        if path.is_dir() && path.join("SKILL.md").is_file() {
            return Ok(Source {
                name: file_name,
                kind: TrackedKind::Skill,
                path,
                managed: false,
                from_tool: false,
            });
        }
        if path.is_file() && file_name.ends_with(".md") && file_name != "SKILL.md" {
            return Ok(Source {
                name: file_name.trim_end_matches(".md").to_string(),
                kind: TrackedKind::Agent,
                path,
                managed: false,
                from_tool: false,
            });
        }
        return Err(AppError::Other(format!(
            "{} is neither a skill directory (with SKILL.md) nor an agent .md file",
            path.display()
        )));
    }

    if cfg.find(source, TrackedKind::Skill).is_some() {
        return Ok(Source {
            name: source.to_string(),
            kind: TrackedKind::Skill,
            path: config::shared_skills_dir().join(source),
            managed: true,
            from_tool: false,
        });
    }
    if cfg.find(source, TrackedKind::Agent).is_some() {
        return Ok(Source {
            name: source.to_string(),
            kind: TrackedKind::Agent,
            path: config::shared_agents_dir().join(format!("{source}.md")),
            managed: true,
            from_tool: false,
        });
    }
    Err(AppError::Other(format!(
        "'{source}' is not a path or a managed skill or agent"
    )))
}

/// Resolve `name` inside one tool's directory of `project`: a skill dir with
/// SKILL.md first, then an agent `.md` file.
fn resolve_in_tool(slug: &str, project: &Path, name: &str) -> Result<Source> {
    if let Ok(dir) = project_dir_for(slug, TrackedKind::Skill, project) {
        let path = dir.join(name);
        if path.join("SKILL.md").is_file() {
            return Ok(Source {
                name: name.to_string(),
                kind: TrackedKind::Skill,
                path,
                managed: false,
                from_tool: true,
            });
        }
    }
    if let Ok(dir) = project_dir_for(slug, TrackedKind::Agent, project) {
        let path = dir.join(format!("{name}.md"));
        if path.is_file() {
            return Ok(Source {
                name: name.to_string(),
                kind: TrackedKind::Agent,
                path,
                managed: false,
                from_tool: true,
            });
        }
    }
    Err(AppError::Other(format!(
        "no skill or agent named '{name}' in {slug}'s directory of {}",
        project.display()
    )))
}

/// Where a transfer reads from when `--from <tool>` is given: the tool, and
/// the project whose tool directory holds the source.
pub struct FromTool<'a> {
    pub slug: &'a str,
    pub project: &'a Path,
}

/// The directory a tool reads `kind` resources from inside `project`: the
/// tool's home-relative directory re-rooted at the project. Tools whose user
/// directory has no project equivalent (under `.config` or `Library`) are
/// rejected rather than guessed.
fn project_dir_for(slug: &str, kind: TrackedKind, project: &Path) -> Result<PathBuf> {
    let tool =
        tools::find_tool(slug).ok_or_else(|| AppError::Other(format!("unknown tool '{slug}'")))?;
    let user_dir = match kind {
        TrackedKind::Skill => tool.skills_dir(),
        TrackedKind::Agent => tool.agents_dir(),
        _ => None,
    }
    .ok_or_else(|| AppError::Other(format!("{slug} has no {kind} directory")))?;
    let rel = user_dir
        .strip_prefix(config::home_dir())
        .map_err(|_| AppError::Other(format!("{slug} {kind} directory is not under $HOME")))?;
    let first = rel
        .components()
        .next()
        .map(|c| c.as_os_str().to_string_lossy().to_string())
        .unwrap_or_default();
    if first == ".config" || first == "Library" {
        return Err(AppError::Other(format!(
            "{slug} has no project-level {kind} directory; pick another tool with --tools"
        )));
    }
    Ok(project.join(rel))
}

/// Copy `source` into `project` for each tool in `tool_slugs`. Refuses to
/// overwrite. `from` reads `source` as a name in one tool's project directory
/// (the same project is allowed, e.g. Claude Code -> Codex). With
/// `move_source`, a managed or `--from` source is removed afterwards. Returns
/// the destination paths written.
pub fn transfer(
    cfg: &mut Config,
    source: &str,
    project: &Path,
    tool_slugs: &[String],
    from: Option<FromTool<'_>>,
    move_source: bool,
) -> Result<Vec<PathBuf>> {
    if !project.is_dir() {
        return Err(AppError::Other(format!(
            "project directory {} does not exist",
            project.display()
        )));
    }
    let src = match from {
        Some(f) => resolve_in_tool(f.slug, f.project, source)?,
        None => resolve_source(cfg, source)?,
    };
    if move_source && !src.managed && !src.from_tool {
        return Err(AppError::Other(
            "--move applies to a managed resource or a --from source; originals at a path are never deleted"
                .into(),
        ));
    }

    let mut dests = Vec::new();
    for slug in tool_slugs {
        let dir = project_dir_for(slug, src.kind, project)?;
        let dest = match src.kind {
            TrackedKind::Skill => dir.join(&src.name),
            _ => dir.join(format!("{}.md", src.name)),
        };
        if dest == src.path {
            return Err(AppError::Other(format!(
                "{slug} already reads {}; pick a different tool with --tools",
                dest.display()
            )));
        }
        if dest.exists() {
            return Err(AppError::Other(format!(
                "{} already exists; remove it first",
                dest.display()
            )));
        }
        dests.push(dest);
    }

    for dest in &dests {
        match src.kind {
            TrackedKind::Skill => manage::copy_dir(&src.path, dest)?,
            _ => {
                if let Some(parent) = dest.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::copy(&src.path, dest)?;
            }
        }
    }

    if move_source {
        if src.managed {
            match src.kind {
                TrackedKind::Skill => remove::remove_skill(cfg, &src.name)?,
                _ => remove::remove_agent(cfg, &src.name)?,
            }
        } else if src.kind == TrackedKind::Skill {
            std::fs::remove_dir_all(&src.path)?;
        } else {
            std::fs::remove_file(&src.path)?;
        }
    }
    Ok(dests)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEFAULT_TOOL: &str = "claude-code";

    fn write_skill(dir: &Path) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(
            dir.join("SKILL.md"),
            "---\nname: x\ndescription: y\n---\nbody\n",
        )
        .unwrap();
    }

    #[test]
    fn transfers_a_skill_between_projects() {
        let tmp = tempfile::tempdir().unwrap();
        let from = tmp.path().join("a").join("skills").join("deploy");
        write_skill(&from);
        let to = tmp.path().join("b");
        std::fs::create_dir_all(&to).unwrap();

        let mut cfg = Config::empty();
        let dests = transfer(
            &mut cfg,
            from.to_str().unwrap(),
            &to,
            &[DEFAULT_TOOL.to_string()],
            None,
            false,
        )
        .unwrap();
        let rel = dests[0].strip_prefix(&to).unwrap().to_path_buf();
        assert_eq!(rel, PathBuf::from(".claude/skills/deploy"));
        assert!(dests[0].join("SKILL.md").is_file());
        // The original is untouched.
        assert!(from.join("SKILL.md").is_file());

        // A second transfer refuses to overwrite.
        let again = transfer(
            &mut cfg,
            from.to_str().unwrap(),
            &to,
            &[DEFAULT_TOOL.to_string()],
            None,
            false,
        );
        assert!(again.is_err());
    }

    #[test]
    fn moves_a_skill_between_tools_in_the_same_project() {
        let tmp = tempfile::tempdir().unwrap();
        let proj = tmp.path();
        let claude = proj.join(".claude").join("skills").join("deploy");
        write_skill(&claude);

        let mut cfg = Config::empty();
        let from = || {
            Some(FromTool {
                slug: DEFAULT_TOOL,
                project: proj,
            })
        };

        // Same tool on both sides is refused, not silently a no-op.
        let same = transfer(
            &mut cfg,
            "deploy",
            proj,
            &[DEFAULT_TOOL.to_string()],
            from(),
            false,
        );
        assert!(same.is_err());

        let dests = transfer(
            &mut cfg,
            "deploy",
            proj,
            &["codex".to_string()],
            from(),
            true,
        )
        .unwrap();
        assert_eq!(dests, [proj.join(".codex").join("skills").join("deploy")]);
        assert!(dests[0].join("SKILL.md").is_file());
        assert!(!claude.exists(), "--move removes the --from source");
    }

    #[test]
    fn move_requires_a_managed_source() {
        let tmp = tempfile::tempdir().unwrap();
        let from = tmp.path().join("skills").join("deploy");
        write_skill(&from);
        let mut cfg = Config::empty();
        let err = transfer(
            &mut cfg,
            from.to_str().unwrap(),
            tmp.path(),
            &[DEFAULT_TOOL.to_string()],
            None,
            true,
        );
        assert!(err.is_err());
        assert!(from.join("SKILL.md").is_file());
    }

    #[test]
    fn rejects_unknown_sources() {
        let tmp = tempfile::tempdir().unwrap();
        let mut cfg = Config::empty();
        assert!(
            transfer(
                &mut cfg,
                "no-such-skill",
                tmp.path(),
                &[DEFAULT_TOOL.to_string()],
                None,
                false,
            )
            .is_err()
        );
    }
}
