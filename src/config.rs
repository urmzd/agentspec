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

pub fn claude_projects_dir() -> PathBuf {
    home_dir().join(".claude").join("projects")
}

pub fn ensure_dirs() -> std::io::Result<()> {
    std::fs::create_dir_all(shared_skills_dir())?;
    std::fs::create_dir_all(shared_agents_dir())?;
    std::fs::create_dir_all(config_dir())?;
    Ok(())
}
