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

#[allow(dead_code)]
pub fn agent_lock_file_path() -> PathBuf {
    agents_base_dir().join(".agent-lock.json")
}

pub fn ensure_dirs() -> std::io::Result<()> {
    std::fs::create_dir_all(shared_skills_dir())?;
    std::fs::create_dir_all(shared_agents_dir())?;
    Ok(())
}
