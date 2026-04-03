use std::path::Path;

use console::style;

use crate::adapters;
use crate::error::{AppError, Result};

pub fn validate(path: Option<&str>) -> Result<()> {
    let path = resolve_path(path)?;

    let adapter = adapters::adapter_for_path(&path)
        .ok_or_else(|| AppError::Other(format!("no adapter for {}", path.display())))?;

    let resource = adapter.parse(&path)?;
    let issues = adapter.validate(&resource);

    if issues.is_empty() {
        println!(
            "  {} {} '{}' is valid ({})",
            style("✓").green().bold(),
            resource.kind,
            resource.name,
            adapter.vendor()
        );
    } else {
        println!(
            "  {} {} '{}' has {} issue(s):",
            style("✗").red().bold(),
            resource.kind,
            resource.name,
            issues.len()
        );
        for issue in &issues {
            println!("    {} {issue}", style("•").red());
        }
    }

    Ok(())
}

fn resolve_path(path: Option<&str>) -> Result<std::path::PathBuf> {
    match path {
        Some(p) => {
            let p = Path::new(p);
            if p.is_file() {
                Ok(p.to_path_buf())
            } else if p.is_dir() {
                let skill_md = p.join("SKILL.md");
                if skill_md.exists() {
                    Ok(skill_md)
                } else {
                    // Look for a single .md file
                    let mds: Vec<_> = std::fs::read_dir(p)?
                        .filter_map(|e| e.ok())
                        .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("md"))
                        .collect();
                    if mds.len() == 1 {
                        Ok(mds[0].path())
                    } else {
                        Err(AppError::Other(format!(
                            "directory {} has no SKILL.md and {} .md files",
                            p.display(),
                            mds.len()
                        )))
                    }
                }
            } else {
                Err(AppError::Other(format!("path not found: {}", p.display())))
            }
        }
        None => {
            let cwd = std::env::current_dir()?;
            let skill_md = cwd.join("SKILL.md");
            if skill_md.exists() {
                Ok(skill_md)
            } else {
                Err(AppError::Other("no SKILL.md in current directory".into()))
            }
        }
    }
}
