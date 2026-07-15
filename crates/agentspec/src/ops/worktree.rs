//! Git worktree management following the local `use-worktrees` policy.
//!
//! Every managed worktree lives under the repository's primary checkout at
//! `<repo-root>/.worktrees/<name>`. The primary checkout is never switched.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;

use crate::error::{AppError, Result};

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct WorktreeEntry {
    pub path: PathBuf,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub head: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    pub detached: bool,
    pub bare: bool,
}

#[derive(Debug, Serialize)]
pub struct CreatedWorktree {
    pub path: PathBuf,
    pub branch: String,
    pub base: String,
    pub created: bool,
}

#[derive(Debug, Serialize)]
struct RemovedWorktree {
    path: PathBuf,
    #[serde(skip_serializing_if = "Option::is_none")]
    branch: Option<String>,
    branch_deleted: bool,
}

fn git(repo: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::Git(stderr.trim().to_string()));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn git_args(repo: &Path, args: &[String]) -> Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::Git(stderr.trim().to_string()));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn repo_path(repo: Option<&str>) -> Result<PathBuf> {
    match repo {
        Some(path) => Ok(PathBuf::from(path)),
        None => Ok(std::env::current_dir()?),
    }
}

fn primary_root(repo: Option<&str>) -> Result<PathBuf> {
    let repo = repo_path(repo)?;
    let common_dir = git(
        &repo,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )?;
    let common_dir = PathBuf::from(common_dir.trim());
    common_dir
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| AppError::Git(format!("cannot derive repository root from {common_dir:?}")))
}

fn ensure_worktrees_ignored(root: &Path) -> Result<()> {
    if Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["check-ignore", "-q", ".worktrees"])
        .status()?
        .success()
    {
        return Ok(());
    }

    let common_dir = git(
        root,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )?;
    let exclude = PathBuf::from(common_dir.trim())
        .join("info")
        .join("exclude");
    if let Some(parent) = exclude.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let existing = std::fs::read_to_string(&exclude).unwrap_or_default();
    if !existing.lines().any(|line| line.trim() == ".worktrees/") {
        let mut next = existing;
        if !next.is_empty() && !next.ends_with('\n') {
            next.push('\n');
        }
        next.push_str(".worktrees/\n");
        std::fs::write(&exclude, next)?;
    }
    Ok(())
}

fn default_base(root: &Path) -> String {
    git(
        root,
        &["symbolic-ref", "-q", "--short", "refs/remotes/origin/HEAD"],
    )
    .ok()
    .map(|s| s.trim().to_string())
    .filter(|s| !s.is_empty())
    .unwrap_or_else(|| "HEAD".to_string())
}

fn safe_name(name: &str) -> Result<String> {
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.contains('/')
        || name.contains('\\')
        || name.chars().any(char::is_whitespace)
    {
        return Err(AppError::Other(format!(
            "worktree name must be a single non-whitespace path component: {name}"
        )));
    }
    Ok(name.to_string())
}

pub fn list(repo: Option<&str>, json: bool) -> Result<()> {
    let root = primary_root(repo)?;
    let entries = parse_worktree_list(&git(&root, &["worktree", "list", "--porcelain"])?);
    if json {
        println!("{}", serde_json::to_string_pretty(&entries)?);
    } else if entries.is_empty() {
        eprintln!("No worktrees found");
    } else {
        for entry in entries {
            let branch = entry
                .branch
                .as_deref()
                .or(entry.head.as_deref())
                .unwrap_or("unknown");
            println!("{} | {}", entry.path.display(), branch);
        }
    }
    Ok(())
}

pub fn create(
    name: &str,
    repo: Option<&str>,
    branch: Option<&str>,
    base: Option<&str>,
    json: bool,
) -> Result<()> {
    let created = ensure(name, repo, branch, base)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&created)?);
    } else {
        let verb = if created.created { "Created" } else { "Using" };
        eprintln!(
            "  {} {verb} worktree {} on {}",
            console::style("✓").green().bold(),
            created.path.display(),
            created.branch
        );
    }
    Ok(())
}

pub fn ensure(
    name: &str,
    repo: Option<&str>,
    branch: Option<&str>,
    base: Option<&str>,
) -> Result<CreatedWorktree> {
    let root = primary_root(repo)?;
    let name = safe_name(name)?;
    ensure_worktrees_ignored(&root)?;

    let wt_root = root.join(".worktrees");
    std::fs::create_dir_all(&wt_root)?;

    let path = wt_root.join(&name);
    let branch = branch
        .map(str::to_string)
        .unwrap_or_else(|| format!("worktree-{name}"));
    let base = base
        .map(str::to_string)
        .unwrap_or_else(|| default_base(&root));

    if path.exists() {
        let current_branch = git(&path, &["branch", "--show-current"])
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| branch.clone());
        return Ok(CreatedWorktree {
            path,
            branch: current_branch,
            base,
            created: false,
        });
    }

    let args = vec![
        "worktree".to_string(),
        "add".to_string(),
        "-b".to_string(),
        branch.clone(),
        path.to_string_lossy().to_string(),
        base.clone(),
    ];
    git_args(&root, &args)?;

    Ok(CreatedWorktree {
        path,
        branch,
        base,
        created: true,
    })
}

pub fn remove(
    target: &str,
    repo: Option<&str>,
    force: bool,
    delete_branch: bool,
    json: bool,
) -> Result<()> {
    let root = primary_root(repo)?;
    let path = resolve_target(&root, target);
    if !path.exists() {
        return Err(AppError::Other(format!(
            "worktree not found: {}",
            path.display()
        )));
    }

    let branch = git(&path, &["branch", "--show-current"])
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let mut args = vec!["worktree".to_string(), "remove".to_string()];
    if force {
        args.push("--force".to_string());
    }
    args.push(path.to_string_lossy().to_string());
    git_args(&root, &args)?;

    let mut branch_deleted = false;
    if let Some(branch) = branch.as_deref()
        && (delete_branch || branch.starts_with("worktree-"))
    {
        let _ = git(&root, &["branch", "-D", branch])?;
        branch_deleted = true;
    }

    let removed = RemovedWorktree {
        path,
        branch,
        branch_deleted,
    };
    if json {
        println!("{}", serde_json::to_string_pretty(&removed)?);
    } else {
        eprintln!(
            "  {} Removed worktree {}",
            console::style("✓").green().bold(),
            removed.path.display()
        );
    }
    Ok(())
}

fn resolve_target(root: &Path, target: &str) -> PathBuf {
    let path = PathBuf::from(target);
    if path.components().count() == 1 {
        root.join(".worktrees").join(path)
    } else {
        path
    }
}

pub fn parse_worktree_list(s: &str) -> Vec<WorktreeEntry> {
    let mut entries = Vec::new();
    let mut current: Option<WorktreeEntry> = None;

    for line in s.lines() {
        if line.is_empty() {
            if let Some(entry) = current.take() {
                entries.push(entry);
            }
            continue;
        }

        if let Some(path) = line.strip_prefix("worktree ") {
            if let Some(entry) = current.take() {
                entries.push(entry);
            }
            current = Some(WorktreeEntry {
                path: PathBuf::from(path),
                head: None,
                branch: None,
                detached: false,
                bare: false,
            });
        } else if let Some(entry) = current.as_mut() {
            if let Some(head) = line.strip_prefix("HEAD ") {
                entry.head = Some(head.to_string());
            } else if let Some(branch) = line.strip_prefix("branch ") {
                entry.branch = Some(branch.trim_start_matches("refs/heads/").to_string());
            } else if line == "detached" {
                entry.detached = true;
            } else if line == "bare" {
                entry.bare = true;
            }
        }
    }

    if let Some(entry) = current {
        entries.push(entry);
    }

    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_porcelain_worktree_list() {
        let entries = parse_worktree_list(
            "worktree /repo\n\
             HEAD abc123\n\
             branch refs/heads/main\n\
             \n\
             worktree /repo/.worktrees/api\n\
             HEAD def456\n\
             branch refs/heads/worktree-api\n\
             \n",
        );

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].path, PathBuf::from("/repo"));
        assert_eq!(entries[0].branch.as_deref(), Some("main"));
        assert_eq!(entries[1].branch.as_deref(), Some("worktree-api"));
    }

    #[test]
    fn rejects_unsafe_names() {
        assert!(safe_name("api").is_ok());
        assert!(safe_name("../api").is_err());
        assert!(safe_name("api branch").is_err());
        assert!(safe_name("").is_err());
    }
}
