use std::path::PathBuf;

pub fn home_dir() -> PathBuf {
    dirs::home_dir().expect("could not determine home directory")
}

pub fn agents_base_dir() -> PathBuf {
    home_dir().join(".agents")
}

pub fn shared_skills_dir() -> PathBuf {
    agents_base_dir().join("skills")
}

pub fn shared_agents_dir() -> PathBuf {
    agents_base_dir().join("agents")
}

pub fn lock_file_path() -> PathBuf {
    agents_base_dir().join(".skill-lock.json")
}

pub fn inventory_lockfile_path() -> PathBuf {
    agents_base_dir().join("agentspec-lock.yml")
}

pub fn config_dir() -> PathBuf {
    dirs::config_dir()
        .expect("could not determine config directory")
        .join("agentspec")
}

pub fn config_path() -> PathBuf {
    config_dir().join("config.yml")
}

pub fn shared_projects_dir() -> PathBuf {
    agents_base_dir().join("projects")
}

pub fn shared_memories_dir() -> PathBuf {
    agents_base_dir().join("memories")
}

pub fn shared_plans_dir() -> PathBuf {
    agents_base_dir().join("plans")
}

pub fn shared_sessions_dir() -> PathBuf {
    agents_base_dir().join("sessions")
}

pub fn shared_mcp_dir() -> PathBuf {
    agents_base_dir().join("mcp")
}

pub fn shared_hooks_dir() -> PathBuf {
    agents_base_dir().join("hooks")
}

pub fn permissions_file() -> PathBuf {
    agents_base_dir().join("permissions.yml")
}

pub fn plugins_manifest() -> PathBuf {
    agents_base_dir().join("plugins.yml")
}

pub fn shared_project_dir(name: &str) -> PathBuf {
    shared_projects_dir().join(name)
}

pub fn claude_projects_dir() -> PathBuf {
    home_dir().join(".claude").join("projects")
}

pub fn ensure_dirs() -> std::io::Result<()> {
    std::fs::create_dir_all(shared_skills_dir())?;
    std::fs::create_dir_all(shared_agents_dir())?;
    std::fs::create_dir_all(shared_projects_dir())?;
    std::fs::create_dir_all(shared_memories_dir())?;
    std::fs::create_dir_all(shared_plans_dir())?;
    std::fs::create_dir_all(shared_sessions_dir())?;
    std::fs::create_dir_all(shared_mcp_dir())?;
    std::fs::create_dir_all(shared_hooks_dir())?;
    std::fs::create_dir_all(config_dir())?;
    Ok(())
}
