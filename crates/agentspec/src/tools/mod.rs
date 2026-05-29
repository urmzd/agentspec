use std::path::PathBuf;

use crate::config;

pub trait CodingTool: Send + Sync {
    fn name(&self) -> &str;
    fn slug(&self) -> &str;
    fn skills_dir(&self) -> Option<PathBuf>;
    fn agents_dir(&self) -> Option<PathBuf>;
    /// Path to the tool's settings file where mcpServers can be registered.
    fn mcp_config_path(&self) -> Option<PathBuf> {
        None
    }
    /// Path to the tool's main settings file (e.g. where permission allowlists live).
    fn settings_path(&self) -> Option<PathBuf> {
        None
    }
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
        .filter_map(|e| {
            let path = e.path();
            let stem = path.file_stem()?.to_string_lossy().to_string();
            Some(stem)
        })
        .collect()
}

macro_rules! define_tool {
    ($struct_name:ident, $name:expr, $slug:expr, skills: $skills:expr, agents: $agents:expr) => {
        define_tool!($struct_name, $name, $slug, skills: $skills, agents: $agents, mcp_config: None, settings_config: None);
    };
    ($struct_name:ident, $name:expr, $slug:expr, skills: $skills:expr, agents: $agents:expr, mcp_config: $mcp:expr) => {
        define_tool!($struct_name, $name, $slug, skills: $skills, agents: $agents, mcp_config: $mcp, settings_config: None);
    };
    ($struct_name:ident, $name:expr, $slug:expr, skills: $skills:expr, agents: $agents:expr, mcp_config: $mcp:expr, settings_config: $settings:expr) => {
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
            fn mcp_config_path(&self) -> Option<PathBuf> {
                let p: Option<&str> = $mcp;
                p.map(|s| config::home_dir().join(s))
            }
            fn settings_path(&self) -> Option<PathBuf> {
                let p: Option<&str> = $settings;
                p.map(|s| config::home_dir().join(s))
            }
        }
    };
}

define_tool!(Claude,    "Claude Code",     "claude-code",     skills: Some(".claude/skills"),           agents: Some(".claude/agents"),           mcp_config: Some(".claude/settings.json"), settings_config: Some(".claude/settings.json"));
define_tool!(Cline,     "Cline",           "cline",           skills: Some(".cline/skills"),            agents: Some(".cline/agents"));
define_tool!(Windsurf,  "Windsurf",        "windsurf",        skills: Some(".codeium/windsurf/skills"), agents: Some(".codeium/windsurf/agents"));
define_tool!(OpenHands, "OpenHands",       "openhands",       skills: Some(".openhands/skills"),        agents: Some(".openhands/agents"));
define_tool!(Gemini,    "Gemini CLI",      "gemini-cli",      skills: Some(".gemini/skills"),           agents: Some(".gemini/agents"),           mcp_config: Some(".gemini/settings.json"), settings_config: Some(".gemini/settings.json"));
define_tool!(Copilot,   "GitHub Copilot",  "github-copilot",  skills: Some(".copilot/skills"),          agents: Some(".copilot/agents"));
define_tool!(Amp,       "Amp",             "amp",             skills: Some(".amp/skills"),              agents: Some(".amp/agents"));
define_tool!(Cursor,    "Cursor",          "cursor",          skills: Some(".cursor/skills"),           agents: Some(".cursor/agents"),           mcp_config: Some(".cursor/mcp.json"));
define_tool!(Codex,     "Codex",           "codex",           skills: Some(".codex/skills"),            agents: Some(".codex/agents"));
define_tool!(OpenCode,  "OpenCode",        "opencode",        skills: Some(".opencode/skills"),         agents: Some(".opencode/agents"));
define_tool!(Kimi,      "Kimi CLI",        "kimi-cli",        skills: Some(".kimi-cli/skills"),         agents: Some(".kimi-cli/agents"));

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
