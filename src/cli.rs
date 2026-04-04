use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "agentspec",
    about = "Universal agent skill & sub-agent manager"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Output as JSON
    #[arg(long, global = true)]
    pub json: bool,
}

#[derive(Subcommand)]
pub enum Command {
    /// Manage skills
    Skill {
        #[command(subcommand)]
        action: SkillAction,
    },
    /// Manage sub-agents
    Agent {
        #[command(subcommand)]
        action: AgentAction,
    },
    /// List detected AI coding tools
    Tool {
        #[command(subcommand)]
        action: ToolAction,
    },
    /// Search skills and agents
    Search {
        /// Search query
        query: String,
        /// Max results
        #[arg(long, default_value = "20")]
        limit: usize,
    },
    /// Manage AI coding sessions
    Session {
        #[command(subcommand)]
        action: SessionAction,
    },
    /// Launch interactive TUI
    Tui,
}

#[derive(Subcommand)]
pub enum SkillAction {
    /// Install a skill from GitHub or local path
    Install {
        /// Source: owner/repo or local path
        source: String,
        /// Link to specific tools (comma-separated slugs)
        #[arg(long, value_delimiter = ',')]
        tools: Option<Vec<String>>,
        /// Link to all detected tools
        #[arg(long)]
        all_tools: bool,
    },
    /// Remove an installed skill
    Remove {
        /// Skill name
        name: String,
    },
    /// List installed skills
    List {
        /// Filter by linked tool
        #[arg(long)]
        tool: Option<String>,
    },
    /// Link a skill to a tool
    Link {
        /// Skill name
        skill: String,
        /// Tool slug
        tool: String,
    },
    /// Unlink a skill from a tool
    Unlink {
        /// Skill name
        skill: String,
        /// Tool slug
        tool: String,
    },
    /// Validate a SKILL.md file
    Validate {
        /// Path to skill directory or SKILL.md (default: current dir)
        path: Option<String>,
    },
    /// Create a new skill from template
    Create {
        /// Skill name
        name: Option<String>,
    },
    /// Update installed skills
    Update {
        /// Skill name (all if omitted)
        name: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum AgentAction {
    /// Install a sub-agent from GitHub or local path
    Install {
        source: String,
        #[arg(long, value_delimiter = ',')]
        tools: Option<Vec<String>>,
        #[arg(long)]
        all_tools: bool,
    },
    /// Remove a sub-agent
    Remove { name: String },
    /// List installed sub-agents
    List {
        #[arg(long)]
        tool: Option<String>,
    },
    /// Link a sub-agent to a tool
    Link { agent: String, tool: String },
    /// Unlink a sub-agent from a tool
    Unlink { agent: String, tool: String },
    /// Validate an agent definition
    Validate { path: Option<String> },
    /// Create a new agent from template
    Create { name: Option<String> },
}

#[derive(Subcommand)]
pub enum ToolAction {
    /// List detected AI coding tools
    List,
}

#[derive(Subcommand)]
pub enum SessionAction {
    /// List sessions for a source
    List {
        /// Source to list (claude, codex)
        source: String,
    },
    /// Fuzzy-find a session across all sources
    Find,
    /// Export a session as markdown
    Export {
        /// Source (claude, codex)
        source: String,
        /// Session ID (omit if using --last)
        id: Option<String>,
        /// Use the most recent session
        #[arg(long)]
        last: bool,
        /// Write to file instead of stdout
        #[arg(short, long)]
        output: Option<String>,
    },
}
