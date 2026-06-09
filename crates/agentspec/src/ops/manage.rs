use std::path::Path;

use console::style;

use crate::config;
use crate::error::{AppError, Result};
use crate::inventory::{Config, SourceType, TrackedKind, TrackedResource, hash_resource};
use crate::ir::ResourceKind;
use crate::ops::discover;
use crate::ops::link;
use crate::tools;

#[derive(Debug)]
pub(crate) struct GitSource {
    url: String,
    branch: Option<String>,
    subpath: Option<String>,
}

#[derive(Debug)]
pub(crate) enum SourceKind {
    Local(String),
    Git(GitSource),
}

/// Parse `owner/repo[#branch][@subpath]` or explicit URL with the same suffixes.
///
/// Examples:
///   owner/repo                          → default branch, repo root
///   owner/repo@subfolder                → default branch, subfolder
///   owner/repo#branch                   → specific branch, repo root
///   owner/repo#branch@subfolder         → specific branch, subfolder
///   https://host/repo.git#branch@sub    → same for explicit URLs
pub(crate) fn resolve_source(input: &str) -> SourceKind {
    // Local path
    if Path::new(input).exists() {
        return SourceKind::Local(input.to_string());
    }

    // Explicit git URL (https://, git://, ssh://)
    if input.starts_with("https://")
        || input.starts_with("git://")
        || input.starts_with("ssh://")
        || input.ends_with(".git")
        || input.contains(".git#")
        || input.contains(".git@")
    {
        let gs = parse_git_url(input);
        return SourceKind::Git(gs);
    }

    // GitHub shorthand: owner/repo[#branch][@subpath]
    if input.contains('/') {
        let gs = parse_shorthand(input);
        return SourceKind::Git(gs);
    }

    // Fall back to local path (will fail later if it doesn't exist)
    SourceKind::Local(input.to_string())
}

/// Parse explicit URL: `https://host/repo.git[#branch][@subpath]`
fn parse_git_url(input: &str) -> GitSource {
    // Find .git boundary — suffixes come after it
    let git_pos = input.find(".git").unwrap_or(input.len());
    let base_url = format!("{}.git", &input[..git_pos]);
    let suffix = &input[git_pos + 4..]; // everything after ".git"

    let (branch, subpath) = parse_suffixes(suffix);
    GitSource {
        url: base_url,
        branch,
        subpath,
    }
}

/// Parse GitHub shorthand: `owner/repo[#branch][@subpath]`
fn parse_shorthand(input: &str) -> GitSource {
    // Split off #branch and @subpath from the repo identifier
    let (repo_part, branch, subpath) = {
        let (rest, subpath) = if let Some(pos) = input.find('@') {
            (&input[..pos], Some(input[pos + 1..].to_string()))
        } else {
            (input, None)
        };
        // Now check for #branch in the remaining part
        let (repo, branch) = if let Some(pos) = rest.find('#') {
            (&rest[..pos], Some(rest[pos + 1..].to_string()))
        } else {
            (rest, None)
        };
        (repo, branch, subpath)
    };

    GitSource {
        url: format!("https://github.com/{repo_part}.git"),
        branch,
        subpath,
    }
}

/// Parse `[#branch][@subpath]` suffix string (leading `#` or `@` already stripped from the
/// appropriate position by the caller — this receives the raw suffix after `.git`).
fn parse_suffixes(suffix: &str) -> (Option<String>, Option<String>) {
    if suffix.is_empty() {
        return (None, None);
    }
    // suffix starts with '#' for branch or '@' for subpath
    if let Some(rest) = suffix.strip_prefix('#') {
        // #branch[@subpath]
        if let Some(pos) = rest.find('@') {
            let branch = rest[..pos].to_string();
            let subpath = rest[pos + 1..].to_string();
            (Some(branch), Some(subpath))
        } else {
            (Some(rest.to_string()), None)
        }
    } else if let Some(rest) = suffix.strip_prefix('@') {
        // @subpath only (no branch)
        (None, Some(rest.to_string()))
    } else {
        (None, None)
    }
}

pub fn manage(
    cfg: &mut Config,
    source: &str,
    tool_slugs: Option<&[String]>,
    all_tools: bool,
    copy: bool,
) -> Result<()> {
    match resolve_source(source) {
        SourceKind::Local(path) => manage_local(cfg, &path, source, tool_slugs, all_tools, copy),
        SourceKind::Git(gs) => manage_git(cfg, &gs, source, tool_slugs, all_tools, copy),
    }
}

fn manage_local(
    cfg: &mut Config,
    path: &str,
    source: &str,
    tool_slugs: Option<&[String]>,
    all_tools: bool,
    copy: bool,
) -> Result<()> {
    let p = Path::new(path);
    if !p.exists() {
        return Err(AppError::Other(format!("path not found: {path}")));
    }

    let installed = install_from_dir(
        p,
        source,
        cfg,
        tool_slugs,
        all_tools,
        copy,
        SourceType::Local,
    )?;

    eprintln!("\n  {} resource(s) managed from {source}", installed);

    Ok(())
}

fn manage_git(
    cfg: &mut Config,
    gs: &GitSource,
    source: &str,
    tool_slugs: Option<&[String]>,
    all_tools: bool,
    copy: bool,
) -> Result<()> {
    eprintln!("  {} Cloning {source}...", style("↓").cyan().bold());

    let (_tmp, install_dir) = clone_source_to_tempdir(gs)?;

    let installed = install_from_dir(
        &install_dir,
        source,
        cfg,
        tool_slugs,
        all_tools,
        copy,
        SourceType::Git,
    )?;

    eprintln!("\n  {} resource(s) managed from {source}", installed);

    Ok(())
}

/// Shallow-clone a git source into a temp dir and return the temp dir handle
/// (keep it alive while reading) plus the install directory (subpath-adjusted).
pub(crate) fn clone_source_to_tempdir(
    gs: &GitSource,
) -> Result<(tempfile::TempDir, std::path::PathBuf)> {
    let tmp = tempfile::tempdir().map_err(AppError::Io)?;

    let mut cmd = std::process::Command::new("git");
    cmd.args(["clone", "--depth", "1"]);
    if let Some(b) = &gs.branch {
        cmd.args(["--branch", b]);
    }
    cmd.args([gs.url.as_str(), tmp.path().to_str().unwrap()]);

    let status = cmd
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|e| AppError::Git(format!("failed to run git: {e}")))?;

    if !status.success() {
        return Err(AppError::Git(format!("git clone failed for {}", gs.url)));
    }

    let install_dir = if let Some(sub) = &gs.subpath {
        let p = tmp.path().join(sub);
        if !p.exists() {
            return Err(AppError::Other(format!(
                "subpath '{sub}' not found in cloned repo"
            )));
        }
        p
    } else {
        tmp.path().to_path_buf()
    };

    Ok((tmp, install_dir))
}

/// Install skills and agents from a directory, updating the config.
fn install_from_dir(
    dir: &Path,
    source: &str,
    cfg: &mut Config,
    tool_slugs: Option<&[String]>,
    all_tools: bool,
    copy: bool,
    source_type: SourceType,
) -> Result<usize> {
    let mut installed = 0;

    // Find and install skills (directories with SKILL.md)
    for entry in walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_name() != "SKILL.md" {
            continue;
        }
        let skill_dir = entry.path().parent().unwrap();
        let name = skill_dir.file_name().unwrap().to_string_lossy().to_string();
        let dest = config::shared_skills_dir().join(&name);

        if dest.exists() {
            eprintln!("  {} {name} already exists, skipping", style("~").yellow());
            continue;
        }

        copy_dir(skill_dir, &dest)?;

        let hash = hash_resource(TrackedKind::Skill, &dest)?;
        let source_str = match source_type {
            SourceType::Git => source.to_string(),
            SourceType::Local => source.to_string(),
            SourceType::Discovered => "discovered".to_string(),
        };

        let tracked = TrackedResource::new(
            name.clone(),
            TrackedKind::Skill,
            source_str,
            source_type,
            format!("skills/{name}"),
            hash,
        );
        cfg.add(tracked);

        let slugs = resolve_tool_slugs(tool_slugs, all_tools);
        if !slugs.is_empty() {
            link::link_to_tools(cfg, ResourceKind::Skill, &name, &slugs, copy)?;
        }

        eprintln!("  {} Managed skill '{}'", style("✓").green().bold(), name);
        installed += 1;
    }

    // Find and install agents (.md files with frontmatter)
    for entry in walkdir::WalkDir::new(dir)
        .max_depth(3)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        if path.file_name().unwrap().to_str() == Some("SKILL.md") {
            continue;
        }

        if !discover::has_valid_agent_frontmatter(path) {
            continue;
        }

        let name = path.file_stem().unwrap().to_string_lossy().to_string();
        let dest = config::shared_agents_dir().join(format!("{name}.md"));

        if dest.exists() {
            eprintln!("  {} {name} already exists, skipping", style("~").yellow());
            continue;
        }

        std::fs::copy(path, &dest)?;

        let hash = hash_resource(TrackedKind::Agent, &dest)?;
        let source_str = match source_type {
            SourceType::Git => source.to_string(),
            SourceType::Local => source.to_string(),
            SourceType::Discovered => "discovered".to_string(),
        };

        let tracked = TrackedResource::new(
            name.clone(),
            TrackedKind::Agent,
            source_str,
            source_type,
            format!("agents/{name}.md"),
            hash,
        );
        cfg.add(tracked);

        let slugs = resolve_tool_slugs(tool_slugs, all_tools);
        if !slugs.is_empty() {
            link::link_to_tools(cfg, ResourceKind::Agent, &name, &slugs, copy)?;
        }

        eprintln!("  {} Managed agent '{}'", style("✓").green().bold(), name);
        installed += 1;
    }

    Ok(installed)
}

pub(crate) fn resolve_tool_slugs(explicit: Option<&[String]>, all: bool) -> Vec<String> {
    if all {
        tools::installed_tools()
            .iter()
            .map(|t| t.slug().to_string())
            .collect()
    } else {
        explicit.map(|s| s.to_vec()).unwrap_or_default()
    }
}

pub(crate) fn copy_dir(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in walkdir::WalkDir::new(src)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let relative = entry.path().strip_prefix(src).unwrap();
        let target = dst.join(relative);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&target)?;
        } else {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_source_prefers_existing_local_paths() {
        let tmp = tempfile::tempdir().unwrap();
        let local = tmp.path().to_string_lossy().to_string();
        assert!(matches!(resolve_source(&local), SourceKind::Local(_)));
        // No slash and not on disk: falls back to local (errors later).
        assert!(matches!(
            resolve_source("definitely-not-a-repo"),
            SourceKind::Local(_)
        ));
    }

    #[test]
    fn resolve_source_parses_shorthand_and_urls() {
        struct Case {
            input: &'static str,
            url: &'static str,
            branch: Option<&'static str>,
            subpath: Option<&'static str>,
        }
        let cases = [
            Case {
                input: "owner/repo",
                url: "https://github.com/owner/repo.git",
                branch: None,
                subpath: None,
            },
            Case {
                input: "owner/repo@skills/foo",
                url: "https://github.com/owner/repo.git",
                branch: None,
                subpath: Some("skills/foo"),
            },
            Case {
                input: "owner/repo#dev",
                url: "https://github.com/owner/repo.git",
                branch: Some("dev"),
                subpath: None,
            },
            Case {
                input: "owner/repo#dev@skills/foo",
                url: "https://github.com/owner/repo.git",
                branch: Some("dev"),
                subpath: Some("skills/foo"),
            },
            Case {
                input: "https://host.example/group/repo.git#main@sub",
                url: "https://host.example/group/repo.git",
                branch: Some("main"),
                subpath: Some("sub"),
            },
        ];
        for case in cases {
            match resolve_source(case.input) {
                SourceKind::Git(gs) => {
                    assert_eq!(gs.url, case.url, "{}", case.input);
                    assert_eq!(gs.branch.as_deref(), case.branch, "{}", case.input);
                    assert_eq!(gs.subpath.as_deref(), case.subpath, "{}", case.input);
                }
                SourceKind::Local(p) => panic!("{}: parsed as local {p}", case.input),
            }
        }
    }

    #[test]
    fn parse_suffixes_handles_all_combinations() {
        assert_eq!(parse_suffixes(""), (None, None));
        assert_eq!(parse_suffixes("#dev"), (Some("dev".into()), None));
        assert_eq!(parse_suffixes("@sub/dir"), (None, Some("sub/dir".into())));
        assert_eq!(
            parse_suffixes("#dev@sub"),
            (Some("dev".into()), Some("sub".into()))
        );
        assert_eq!(parse_suffixes("junk"), (None, None));
    }

    #[test]
    fn copy_dir_copies_nested_trees() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("src");
        std::fs::create_dir_all(src.join("nested/deep")).unwrap();
        std::fs::write(src.join("top.txt"), "top").unwrap();
        std::fs::write(src.join("nested/deep/leaf.txt"), "leaf").unwrap();

        let dst = tmp.path().join("dst");
        copy_dir(&src, &dst).unwrap();
        assert_eq!(std::fs::read_to_string(dst.join("top.txt")).unwrap(), "top");
        assert_eq!(
            std::fs::read_to_string(dst.join("nested/deep/leaf.txt")).unwrap(),
            "leaf"
        );
    }
}
