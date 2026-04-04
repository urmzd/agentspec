use console::style;

use crate::adapters;
use crate::config;
use crate::error::Result;
use crate::ir::Resource;
use crate::lockfile::LockFile;
use crate::ops::verify;
use crate::tools::{self, CodingTool};

pub fn list_skills(tool_filter: Option<&str>, json: bool) -> Result<()> {
    // Check integrity of managed resources
    if !json
        && let Ok(issues) = verify::verify_integrity()
        && !issues.is_empty()
    {
        verify::warn_integrity_issues(&issues);
        eprintln!();
    }

    let lock = LockFile::load(&config::lock_file_path())?;
    let installed = tools::installed_tools();
    let skills_dir = config::shared_skills_dir();

    let mut resources: Vec<(String, Option<Resource>, Vec<String>)> = Vec::new();

    for name in lock.skills.keys() {
        let skill_dir = skills_dir.join(name);
        let skill_md = skill_dir.join("SKILL.md");
        let resource = if skill_md.exists() {
            adapters::agentskills::AgentSkillsAdapter
                .parse(&skill_md)
                .ok()
        } else {
            None
        };

        let linked_to: Vec<String> = installed
            .iter()
            .filter(|t| t.linked_skills().contains(name))
            .map(|t| t.slug().to_string())
            .collect();

        if let Some(filter) = tool_filter
            && !linked_to.iter().any(|s| s == filter)
        {
            continue;
        }

        resources.push((name.clone(), resource, linked_to));
    }

    resources.sort_by(|a, b| a.0.cmp(&b.0));

    if json {
        let out: Vec<serde_json::Value> = resources
            .iter()
            .map(|(name, res, linked)| {
                serde_json::json!({
                    "name": name,
                    "description": res.as_ref().map(|r| &r.description),
                    "tools": linked,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }

    println!(
        "  {:<20} {:<45} {}",
        style("Name").bold().underlined(),
        style("Description").bold().underlined(),
        style("Tools").bold().underlined()
    );

    for (name, resource, linked) in &resources {
        let desc = resource
            .as_ref()
            .map(|r| truncate(&r.description, 42))
            .unwrap_or_else(|| "?".into());
        let dots = tool_dots(&installed, linked);
        println!("  {:<20} {:<45} {}", style(name).green(), desc, dots);
    }

    println!("\n  {} skills installed", style(resources.len()).bold());
    print_tool_legend(&installed);
    Ok(())
}

fn tool_dots(installed: &[Box<dyn CodingTool>], linked: &[String]) -> String {
    installed
        .iter()
        .map(|t| {
            if linked.contains(&t.slug().to_string()) {
                style("●").green().to_string()
            } else {
                style("○").dim().to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn print_tool_legend(installed: &[Box<dyn CodingTool>]) {
    let legend: Vec<String> = installed
        .iter()
        .map(|t| {
            let s = if t.is_installed() {
                style(t.slug()).green()
            } else {
                style(t.slug()).dim()
            };
            s.to_string()
        })
        .collect();
    println!("  Legend: {}", legend.join("  "));
}

fn truncate(s: &str, max: usize) -> String {
    let first_line = s.lines().next().unwrap_or(s);
    if first_line.len() <= max {
        first_line.to_string()
    } else {
        format!("{}...", &first_line[..max - 3])
    }
}

use crate::adapters::Adapter;