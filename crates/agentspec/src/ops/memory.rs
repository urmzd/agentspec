use std::path::{Path, PathBuf};

use console::style;

use crate::config;
use crate::error::Result;
use crate::inventory;

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct MemoryEntry {
    pub name: String,
    pub description: String,
    pub memory_type: String,
    pub project_name: String,
    pub project_path: Option<String>,
    pub file_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ProjectInfo {
    pub encoded_name: String,
    pub project_path: Option<PathBuf>,
}

// ---------------------------------------------------------------------------
// Project path decoding
// ---------------------------------------------------------------------------

/// Decode a Claude Code project directory name back to a filesystem path.
///
/// Claude encodes `/Users/urmzd/github/agentspec` as `-Users-urmzd-github-agentspec`.
/// We greedily reconstruct by trying `/` at each `-` and checking filesystem existence.
pub fn decode_project_path(encoded: &str) -> Option<PathBuf> {
    if !encoded.starts_with('-') {
        return None;
    }

    let rest = &encoded[1..]; // strip leading '-'
    let parts: Vec<&str> = rest.split('-').collect();
    if parts.is_empty() {
        return None;
    }

    let mut path = PathBuf::from("/");
    let mut i = 0;

    while i < parts.len() {
        // Try longest segment first (greedy: combine parts with '-')
        let mut matched = false;
        for j in (i + 1..=parts.len()).rev() {
            let segment = parts[i..j].join("-");
            let candidate = path.join(&segment);
            if candidate.exists() {
                path = candidate;
                i = j;
                matched = true;
                break;
            }
        }
        if !matched {
            // No match found — use single part and continue
            path = path.join(parts[i]);
            i += 1;
        }
    }

    if path.exists() { Some(path) } else { None }
}

// ---------------------------------------------------------------------------
// Scanning
// ---------------------------------------------------------------------------

/// Scan all Claude Code project directories for project info (AGENTS.md + memories).
pub fn scan_project_infos() -> Vec<ProjectInfo> {
    let projects_dir = config::claude_projects_dir();
    if !projects_dir.exists() {
        return Vec::new();
    }

    let Ok(entries) = std::fs::read_dir(&projects_dir) else {
        return Vec::new();
    };

    let mut infos = Vec::new();

    for entry in entries.filter_map(|e| e.ok()) {
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }

        let encoded_name = dir.file_name().unwrap().to_string_lossy().to_string();
        let project_path = decode_project_path(&encoded_name);

        infos.push(ProjectInfo {
            encoded_name,
            project_path,
        });
    }

    infos.sort_by(|a, b| a.encoded_name.cmp(&b.encoded_name));
    infos
}

/// Scan all Claude Code project directories for memory files.
pub fn scan_memories() -> Vec<MemoryEntry> {
    let projects_dir = config::claude_projects_dir();
    if !projects_dir.exists() {
        return Vec::new();
    }

    let Ok(entries) = std::fs::read_dir(&projects_dir) else {
        return Vec::new();
    };

    let mut memories = Vec::new();

    for entry in entries.filter_map(|e| e.ok()) {
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }

        let encoded_name = dir.file_name().unwrap().to_string_lossy().to_string();
        let project_path =
            decode_project_path(&encoded_name).map(|p| p.to_string_lossy().to_string());

        let memory_dir = dir.join("memory");
        if !memory_dir.exists() {
            continue;
        }

        let Ok(mem_entries) = std::fs::read_dir(&memory_dir) else {
            continue;
        };

        for mem_entry in mem_entries.filter_map(|e| e.ok()) {
            let path = mem_entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }

            let content = match std::fs::read_to_string(&path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            if let Some(entry) =
                parse_memory_file(&content, &path, &encoded_name, project_path.as_deref())
            {
                memories.push(entry);
            }
        }
    }

    memories.sort_by(|a, b| {
        a.project_name
            .cmp(&b.project_name)
            .then(a.name.cmp(&b.name))
    });
    memories
}

fn parse_memory_file(
    content: &str,
    file_path: &Path,
    project_name: &str,
    project_path: Option<&str>,
) -> Option<MemoryEntry> {
    // Quick frontmatter check
    if !content.trim_start().starts_with("---") {
        return None;
    }

    let parsed = crate::frontmatter::parse(content).ok()?;
    let yaml: serde_yaml::Value = serde_yaml::from_str(&parsed.frontmatter).ok()?;
    let map = yaml.as_mapping()?;

    let name = map
        .get(serde_yaml::Value::String("name".into()))?
        .as_str()?
        .to_string();
    let description = map
        .get(serde_yaml::Value::String("description".into()))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let memory_type = map
        .get(serde_yaml::Value::String("type".into()))
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    Some(MemoryEntry {
        name,
        description,
        memory_type,
        project_name: project_name.to_string(),
        project_path: project_path.map(|s| s.to_string()),
        file_path: file_path.to_path_buf(),
    })
}

// ---------------------------------------------------------------------------
// CLI command
// ---------------------------------------------------------------------------

pub fn list_memories(project: Option<&str>, mem_type: Option<&str>, json: bool) -> Result<()> {
    let mut memories = scan_memories();

    // Filter by project (match against encoded name or decoded path)
    if let Some(filter) = project {
        memories.retain(|m| {
            m.project_name.contains(filter)
                || m.project_path.as_ref().is_some_and(|p| p.contains(filter))
        });
    }

    // Filter by memory type
    if let Some(filter) = mem_type {
        memories.retain(|m| m.memory_type == filter);
    }

    if json {
        let out: Vec<serde_json::Value> = memories
            .iter()
            .map(|m| {
                serde_json::json!({
                    "name": m.name,
                    "description": m.description,
                    "type": m.memory_type,
                    "project": m.project_path.as_deref().unwrap_or(&m.project_name),
                    "file": m.file_path.to_string_lossy(),
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }

    if memories.is_empty() {
        println!("  {} No memories found", style("~").yellow());
        return Ok(());
    }

    // Group by project for display
    let mut current_project = String::new();
    for m in &memories {
        let display_project = m.project_path.as_deref().unwrap_or(&m.project_name);

        if display_project != current_project {
            if !current_project.is_empty() {
                println!();
            }
            println!("  {}", style(display_project).bold().underlined());
            current_project = display_project.to_string();
        }

        println!(
            "    {:<30} {:<12} {}",
            style(&m.name).cyan(),
            style(&m.memory_type).dim(),
            truncate(&m.description, 40),
        );
    }

    println!(
        "\n  {} memory file(s) across {} project(s)",
        style(memories.len()).bold(),
        {
            let mut projects: Vec<&str> =
                memories.iter().map(|m| m.project_name.as_str()).collect();
            projects.dedup();
            projects.len()
        }
    );

    Ok(())
}

fn truncate(s: &str, max: usize) -> String {
    let first_line = s.lines().next().unwrap_or(s);
    if first_line.len() <= max {
        first_line.to_string()
    } else {
        format!("{}...", &first_line[..max - 3])
    }
}

// ---------------------------------------------------------------------------
// Cross-tool memory sync
//
// Claude Code is currently the only tool with a memory store
// (`~/.claude/projects/<proj>/memory/`). agentspec keeps a portable canonical
// copy under `~/.agents/memories/<project>/` so memories can be backed up and,
// when other tools gain memory support, mirrored across them.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
pub enum MemorySync {
    /// Copy tool memories into the canonical store.
    Pull,
    /// Copy canonical memories back into a tool's store.
    Push,
}

/// The canonical-store subdirectory for a memory's project. Uses Claude Code's
/// encoded project name, which is globally unique and round-trips on push —
/// unlike the bare folder basename, which collides when two projects share a
/// directory name (e.g. `~/work/app` and `~/personal/app`).
fn project_key(m: &MemoryEntry) -> String {
    m.project_name.clone()
}

/// True if the file is identical to one already at `dest` (content hash).
fn same_content(src: &Path, dest: &Path) -> bool {
    if !dest.exists() {
        return false;
    }
    match (inventory::hash_file(src), inventory::hash_file(dest)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

pub fn sync_memories(direction: MemorySync, project: Option<&str>, json: bool) -> Result<()> {
    match direction {
        MemorySync::Pull => pull_memories(project, json),
        MemorySync::Push => push_memories(project, json),
    }
}

/// Pull Claude Code memories into `~/.agents/memories/<project>/`.
fn pull_memories(project: Option<&str>, json: bool) -> Result<()> {
    let mut memories = scan_memories();
    if let Some(filter) = project {
        memories.retain(|m| {
            m.project_name.contains(filter)
                || m.project_path.as_ref().is_some_and(|p| p.contains(filter))
        });
    }

    let base = config::shared_memories_dir();
    let mut copied: Vec<String> = Vec::new();

    for m in &memories {
        let proj = project_key(m);
        let dest_dir = base.join(&proj);
        std::fs::create_dir_all(&dest_dir)?;
        let fname = m.file_path.file_name().unwrap();
        let dest = dest_dir.join(fname);
        if same_content(&m.file_path, &dest) {
            continue;
        }
        std::fs::copy(&m.file_path, &dest)?;
        copied.push(format!("{proj}/{}", fname.to_string_lossy()));
    }

    report_sync("pulled", &copied, base.display().to_string(), json);
    Ok(())
}

/// Push canonical memories back into matching Claude Code project memory dirs.
fn push_memories(project: Option<&str>, json: bool) -> Result<()> {
    let base = config::shared_memories_dir();
    let infos = scan_project_infos();
    let mut pushed: Vec<String> = Vec::new();

    let Ok(proj_dirs) = std::fs::read_dir(&base) else {
        report_sync("pushed", &pushed, "Claude Code".into(), json);
        return Ok(());
    };

    for proj_entry in proj_dirs.filter_map(|e| e.ok()) {
        let proj_dir = proj_entry.path();
        if !proj_dir.is_dir() {
            continue;
        }
        let proj_name = proj_dir.file_name().unwrap().to_string_lossy().to_string();
        if let Some(filter) = project
            && !proj_name.contains(filter)
        {
            continue;
        }

        // Match by the unique encoded project name (the store subdir key).
        let target = infos.iter().find(|i| i.encoded_name == proj_name);

        let Some(info) = target else {
            eprintln!(
                "  {} no Claude project matches '{proj_name}'; skipping",
                style("~").yellow()
            );
            continue;
        };

        let mem_dir = config::claude_projects_dir()
            .join(&info.encoded_name)
            .join("memory");
        std::fs::create_dir_all(&mem_dir)?;

        for file in std::fs::read_dir(&proj_dir)?.filter_map(|e| e.ok()) {
            let src = file.path();
            if src.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            let fname = src.file_name().unwrap();
            let dest = mem_dir.join(fname);
            if same_content(&src, &dest) {
                continue;
            }
            std::fs::copy(&src, &dest)?;
            pushed.push(format!("{proj_name}/{}", fname.to_string_lossy()));
        }
    }

    report_sync("pushed", &pushed, "Claude Code".into(), json);
    Ok(())
}

fn report_sync(verb: &str, items: &[String], target: String, json: bool) {
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({ "synced": items }))
                .unwrap_or_default()
        );
        return;
    }
    if items.is_empty() {
        println!(
            "  {} nothing to {} (already in sync)",
            style("·").dim(),
            verb
        );
        return;
    }
    for it in items {
        println!("  {} {verb} {it}", style("✓").green().bold());
    }
    println!("\n  {} memory file(s) {verb} → {target}", items.len());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_key_uses_unique_encoded_name() {
        // Two projects that share a folder basename ("app") must NOT collide:
        // the key is the unique encoded name, not the basename.
        let a = MemoryEntry {
            name: "ctx".into(),
            description: String::new(),
            memory_type: "project".into(),
            project_name: "-Users-urmzd-work-app".into(),
            project_path: Some("/Users/urmzd/work/app".into()),
            file_path: PathBuf::from("/x/ctx.md"),
        };
        let b = MemoryEntry {
            project_name: "-Users-urmzd-personal-app".into(),
            project_path: Some("/Users/urmzd/personal/app".into()),
            ..a.clone()
        };
        assert_eq!(project_key(&a), "-Users-urmzd-work-app");
        assert_ne!(project_key(&a), project_key(&b));
    }

    #[test]
    fn same_content_detects_identical_files() {
        let tmp = tempfile::tempdir().unwrap();
        let a = tmp.path().join("a.md");
        let b = tmp.path().join("b.md");
        std::fs::write(&a, "hello").unwrap();
        assert!(!same_content(&a, &b)); // b missing
        std::fs::write(&b, "hello").unwrap();
        assert!(same_content(&a, &b));
        std::fs::write(&b, "different").unwrap();
        assert!(!same_content(&a, &b));
    }
}
