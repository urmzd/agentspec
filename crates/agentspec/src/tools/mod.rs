use std::path::PathBuf;

use crate::config;

/// How a tool stores MCP server definitions inside its own config file.
///
/// agentspec keeps one canonical server shape (see [`crate::mcp::McpServer`])
/// and translates it into whichever dialect the target tool speaks, so a
/// server registered once shows up natively everywhere.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpDialect {
    /// JSON object keyed by server name under a literal top-level key
    /// (`mcpServers`, or Amp's VS Code-style `amp.mcpServers`). Values already
    /// use the canonical shape.
    JsonMap(&'static str),
    /// TOML tables under a prefix, as Codex uses (`[mcp_servers.<name>]`).
    TomlTable(&'static str),
    /// OpenCode's JSON dialect: `{type, command: [argv…], environment, enabled}`.
    OpenCodeJson(&'static str),
}

impl McpDialect {
    /// Short label for `mcp list` / `mcp doctor` output.
    pub fn label(&self) -> &'static str {
        match self {
            McpDialect::JsonMap(_) => "json",
            McpDialect::TomlTable(_) => "toml",
            McpDialect::OpenCodeJson(_) => "opencode-json",
        }
    }

    /// The config key (top-level JSON key, or TOML table prefix) servers live under.
    pub fn key(&self) -> &'static str {
        match self {
            McpDialect::JsonMap(k) | McpDialect::TomlTable(k) | McpDialect::OpenCodeJson(k) => k,
        }
    }
}

/// A resolved MCP write target: which file, in which dialect, for which tool.
#[derive(Debug, Clone)]
pub struct McpTarget {
    pub slug: String,
    pub name: String,
    pub path: PathBuf,
    pub dialect: McpDialect,
}

pub trait CodingTool: Send + Sync {
    fn name(&self) -> &str;
    fn slug(&self) -> &str;
    fn skills_dir(&self) -> Option<PathBuf>;
    fn agents_dir(&self) -> Option<PathBuf>;
    /// Where and how this tool stores MCP servers, if it supports them.
    fn mcp_target(&self) -> Option<McpTarget> {
        None
    }
    /// Path to the tool's settings file where mcpServers can be registered.
    fn mcp_config_path(&self) -> Option<PathBuf> {
        self.mcp_target().map(|t| t.path)
    }
    /// Path to the tool's main settings file (e.g. where permission allowlists live).
    fn settings_path(&self) -> Option<PathBuf> {
        None
    }
    fn is_installed(&self) -> bool {
        [
            self.skills_dir(),
            self.agents_dir(),
            self.mcp_config_path(),
            self.settings_path(),
        ]
        .into_iter()
        .flatten()
        .any(|d| d.parent().is_some_and(|p| p.exists()))
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
        define_tool!($struct_name, $name, $slug, skills: $skills, agents: $agents, mcp: None, settings_config: None);
    };
    ($struct_name:ident, $name:expr, $slug:expr, skills: $skills:expr, agents: $agents:expr, mcp: $mcp:expr) => {
        define_tool!($struct_name, $name, $slug, skills: $skills, agents: $agents, mcp: $mcp, settings_config: None);
    };
    ($struct_name:ident, $name:expr, $slug:expr, skills: $skills:expr, agents: $agents:expr, mcp: $mcp:expr, settings_config: $settings:expr) => {
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
            fn mcp_target(&self) -> Option<McpTarget> {
                let t: Option<(&str, McpDialect)> = $mcp;
                t.map(|(rel, dialect)| McpTarget {
                    slug: $slug.to_string(),
                    name: $name.to_string(),
                    path: config::home_dir().join(rel),
                    dialect,
                })
            }
            fn settings_path(&self) -> Option<PathBuf> {
                let p: Option<&str> = $settings;
                p.map(|s| config::home_dir().join(s))
            }
        }
    };
}

const JSON_MCP: McpDialect = McpDialect::JsonMap("mcpServers");

define_tool!(Claude,    "Claude Code",     "claude-code",     skills: Some(".claude/skills"),           agents: Some(".claude/agents"),           mcp: Some((".claude/settings.json", JSON_MCP)), settings_config: Some(".claude/settings.json"));
define_tool!(Cline,     "Cline",           "cline",           skills: Some(".cline/skills"),            agents: Some(".cline/agents"),            mcp: Some((".cline/mcp_settings.json", JSON_MCP)));
define_tool!(Windsurf,  "Windsurf",        "windsurf",        skills: Some(".codeium/windsurf/skills"), agents: Some(".codeium/windsurf/agents"), mcp: Some((".codeium/windsurf/mcp_config.json", JSON_MCP)));
define_tool!(OpenHands, "OpenHands",       "openhands",       skills: Some(".openhands/skills"),        agents: Some(".openhands/agents"));
define_tool!(Gemini,    "Gemini CLI",      "gemini-cli",      skills: Some(".gemini/skills"),           agents: Some(".gemini/agents"),           mcp: Some((".gemini/settings.json", JSON_MCP)), settings_config: Some(".gemini/settings.json"));
define_tool!(Copilot,   "GitHub Copilot",  "github-copilot",  skills: Some(".copilot/skills"),          agents: Some(".copilot/agents"),          mcp: Some((".copilot/mcp-config.json", JSON_MCP)), settings_config: Some(".copilot/settings.json"));
define_tool!(Amp,       "Amp",             "amp",             skills: Some(".amp/skills"),              agents: Some(".amp/agents"),              mcp: Some((".config/amp/settings.json", McpDialect::JsonMap("amp.mcpServers"))));
define_tool!(Cursor,    "Cursor",          "cursor",          skills: Some(".cursor/skills"),           agents: Some(".cursor/agents"),           mcp: Some((".cursor/mcp.json", JSON_MCP)));
define_tool!(Codex,     "Codex",           "codex",           skills: Some(".codex/skills"),            agents: Some(".codex/agents"),            mcp: Some((".codex/config.toml", McpDialect::TomlTable("mcp_servers"))));
define_tool!(OpenCode,  "OpenCode",        "opencode",        skills: Some(".opencode/skills"),         agents: Some(".opencode/agents"),         mcp: Some((".config/opencode/opencode.json", McpDialect::OpenCodeJson("mcp"))));
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

/// Every tool that can host MCP servers, whether or not it is installed.
pub fn all_mcp_targets() -> Vec<McpTarget> {
    all_tools().iter().filter_map(|t| t.mcp_target()).collect()
}
