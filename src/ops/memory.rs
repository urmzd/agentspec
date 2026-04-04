use std::path::{Path, PathBuf};

use console::style;

use crate::config;
use crate::error::Result;

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
    pub has_agents_md: bool,
    pub has_claude_md: bool,
    pub has_llms_txt: bool,
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

        // Check for project-root config files
        let (has_agents_md, has_claude_md, has_llms_txt) =
            if let Some(ref pp) = project_path {
                (
                    pp.join("AGENTS.md").exists(),
                    pp.join("CLAUDE.md").exists(),
                    pp.join("llms.txt").exists(),
                )
            } else {
                (false, false, false)
            };

        infos.push(ProjectInfo {
            encoded_name,
            project_path,
            has_agents_md,
            has_claude_md,
            has_llms_txt,
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
