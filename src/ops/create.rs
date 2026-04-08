use console::style;

use crate::error::{AppError, Result};

pub fn create_skill(name: Option<&str>) -> Result<()> {
    let name = name
        .map(String::from)
        .unwrap_or_else(|| prompt("Skill name"));

    validate_name(&name)?;

    let dir = std::env::current_dir()?.join(&name);
    if dir.exists() {
        return Err(AppError::AlreadyExists(format!(
            "directory '{name}' already exists"
        )));
    }

    let description = prompt("Description (when should this skill be used?)");

    std::fs::create_dir_all(dir.join("references"))?;
    std::fs::create_dir_all(dir.join("scripts"))?;

    let content = format!(
        "---\nname: {name}\ndescription: |\n  {description}\n---\n\n# {name}\n\n## Instructions\n\n<!-- Add your skill instructions here -->\n"
    );
    std::fs::write(dir.join("SKILL.md"), content)?;

    println!(
        "  {} Created skill scaffold at ./{name}/",
        style("✓").green().bold()
    );
    println!("  Edit {name}/SKILL.md to add your instructions.");
    Ok(())
}

pub fn create_agent(name: Option<&str>) -> Result<()> {
    let name = name
        .map(String::from)
        .unwrap_or_else(|| prompt("Agent name"));

    validate_name(&name)?;

    let file = std::env::current_dir()?.join(format!("{name}.md"));
    if file.exists() {
        return Err(AppError::AlreadyExists(format!(
            "file '{name}.md' already exists"
        )));
    }

    let description = prompt("Description (when should this agent be invoked?)");
    let model = prompt_default("Model (sonnet/opus/haiku/inherit)", "inherit");

    let content = format!(
        "---\nname: {name}\ndescription: |\n  {description}\nmodel: {model}\n---\n\nYou are a specialized agent.\n\n## Instructions\n\n<!-- Add your agent system prompt here -->\n"
    );
    std::fs::write(&file, content)?;

    println!(
        "  {} Created agent definition at ./{name}.md",
        style("✓").green().bold()
    );
    Ok(())
}

pub fn create_project_config(name: Option<&str>) -> Result<()> {
    let variant = name.unwrap_or("AGENTS.md");
    let filename = match variant {
        "claude" | "CLAUDE.md" => "CLAUDE.md",
        "gemini" | "GEMINI.md" => "GEMINI.md",
        _ => "AGENTS.md",
    };

    let file = std::env::current_dir()?.join(filename);
    if file.exists() {
        return Err(AppError::AlreadyExists(format!(
            "{filename} already exists"
        )));
    }

    let project_name = std::env::current_dir()?
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "project".to_string());

    let content = format!(
        "# {project_name}\n\n## Identity\n\nDescribe the project purpose and architecture.\n\n## Code Style\n\n<!-- Add conventions, formatting rules, patterns -->\n\n## Commands\n\n```sh\n# build\n# test\n# lint\n```\n"
    );
    std::fs::write(&file, content)?;

    println!(
        "  {} Created {filename} in current directory",
        style("✓").green().bold()
    );
    Ok(())
}

pub fn create_llms_txt(_name: Option<&str>) -> Result<()> {
    let file = std::env::current_dir()?.join("llms.txt");
    if file.exists() {
        return Err(AppError::AlreadyExists("llms.txt already exists".into()));
    }

    let project_name = std::env::current_dir()?
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "project".to_string());

    let content = format!(
        "# {project_name}\n\n> Brief description of the project.\n\n## Overview\n\nDescribe the project for LLM consumption.\n\n## Quickstart\n\n## API\n\n## Architecture\n"
    );
    std::fs::write(&file, content)?;

    println!(
        "  {} Created llms.txt in current directory",
        style("✓").green().bold()
    );
    Ok(())
}

fn validate_name(name: &str) -> Result<()> {
    if name.is_empty() {
        return Err(AppError::Other("name cannot be empty".into()));
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(AppError::Other(
            "name must contain only lowercase letters, digits, and hyphens".into(),
        ));
    }
    Ok(())
}

fn prompt(label: &str) -> String {
    eprint!("  {}: ", style(label).bold());
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap_or_default();
    input.trim().to_string()
}

fn prompt_default(label: &str, default: &str) -> String {
    eprint!("  {} [{}]: ", style(label).bold(), default);
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap_or_default();
    let trimmed = input.trim();
    if trimmed.is_empty() {
        default.to_string()
    } else {
        trimmed.to_string()
    }
}
