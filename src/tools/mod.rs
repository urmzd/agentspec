use std::path::PathBuf;

use crate::config;

pub trait CodingTool: Send + Sync {
    fn name(&self) -> &str;
    fn slug(&self) -> &str;
    fn skills_dir(&self) -> Option<PathBuf>;
    fn agents_dir(&self) -> Option<PathBuf>;
    fn is_installed(&self) -> bool {
        self.skills_dir()
            .is_some_and(|d| d.parent().is_some_and(|p| p.exists()))
            || self
                .agents_dir()
                .is_some_and(|d| d.parent().is_some_and(|p| p.exists()))
    }
    fn linked_skills(&self) -> Vec<String> {
        self.skills_dir()
            .filter(|d| d.exists())
            .map(|d| list_symlinks(&d))
            .unwrap_or_default()
    }
    fn linked_agents(&self) -> Vec<String> {
        self.agents_dir()
            .filter(|d| d.exists())
            .map(|d| list_symlinks(&d))
            .unwrap_or_default()
    }
}

fn list_symlinks(dir: &std::path::Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_symlink())
        .filter_map(|e| e.file_name().into_string().ok())
        .collect()
}

macro_rules! define_tool {
    ($struct_name:ident, $name:expr, $slug:expr, skills: $skills:expr, agents: $agents:expr) => {
        pub struct $struct_name;
        impl CodingTool for $struct_name {
            fn name(&self) -> &str {
                $name
            }
            fn slug(&self) -> &str {
                $slug
            }
            fn skills_dir(&self) -> Option<PathBuf> {
                let p: Option<&str> = $skills;
                p.map(|s| config::home_dir().join(s))
            }
            fn agents_dir(&self) -> Option<PathBuf> {
                let p: Option<&str> = $agents;
                p.map(|s| config::home_dir().join(s))
            }
        }
    };
}

define_tool!(Claude,    "Claude Code",     "claude-code",     skills: Some(".claude/skills"),           agents: Some(".claude/agents"));
define_tool!(Cline,     "Cline",           "cline",           skills: Some(".cline/skills"),            agents: None);
define_tool!(Windsurf,  "Windsurf",        "windsurf",        skills: Some(".codeium/windsurf/skills"), agents: None);
define_tool!(OpenHands, "OpenHands",       "openhands",       skills: Some(".openhands/skills"),        agents: None);
define_tool!(Gemini,    "Gemini CLI",      "gemini-cli",      skills: Some(".gemini/skills"),           agents: Some(".gemini/agents"));
define_tool!(Copilot,   "GitHub Copilot",  "github-copilot",  skills: Some(".copilot/skills"),          agents: None);
define_tool!(Amp,       "Amp",             "amp",             skills: Some(".amp/skills"),              agents: None);
define_tool!(Cursor,    "Cursor",          "cursor",          skills: Some(".cursor/skills"),           agents: None);
define_tool!(Codex,     "Codex",           "codex",           skills: Some(".codex/skills"),            agents: None);
define_tool!(OpenCode,  "OpenCode",        "opencode",        skills: Some(".opencode/skills"),         agents: None);
define_tool!(Kimi,      "Kimi CLI",        "kimi-cli",        skills: Some(".kimi-cli/skills"),         agents: None);

pub fn all_tools() -> Vec<Box<dyn CodingTool>> {
    vec![
        Box::new(Claude),
        Box::new(Cline),
        Box::new(Windsurf),
        Box::new(OpenHands),
        Box::new(Gemini),
        Box::new(Copilot),
        Box::new(Amp),
        Box::new(Cursor),
        Box::new(Codex),
        Box::new(OpenCode),
        Box::new(Kimi),
    ]
}

pub fn installed_tools() -> Vec<Box<dyn CodingTool>> {
    all_tools()
        .into_iter()
        .filter(|t| t.is_installed())
        .collect()
}

pub fn find_tool(slug: &str) -> Option<Box<dyn CodingTool>> {
    all_tools().into_iter().find(|t| t.slug() == slug)
}
