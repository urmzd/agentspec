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
    /// Manage resources (skills, agents, memories)
    Manage {
        #[command(subcommand)]
        action: ManageAction,
    },
    /// Show managed and unmanaged resource inventory
    Status,
    /// Manage AI coding sessions
    Session {
        #[command(subcommand)]
        action: SessionAction,
    },
    /// Launch interactive TUI
    Tui,
}

#[derive(Subcommand)]
pub enum ManageAction {
    /// Add a resource (local path, git URL, owner/repo, or discovered name)
    Add {
        /// Source: local path, git URL, owner/repo, or name of a discovered resource
        source: String,
        /// Override auto-detected kind
        #[arg(long, value_parser = ["skill", "agent", "session", "memory", "project-config", "llms-txt"])]
        kind: Option<String>,
        /// Link to specific tools (comma-separated slugs)
        #[arg(long, value_delimiter = ',')]
        tools: Option<Vec<String>>,
        /// Link to all detected tools
        #[arg(long)]
        all_tools: bool,
        /// Copy to tool dirs instead of symlinking
        #[arg(long)]
        copy: bool,
    },
    /// Remove a managed resource
    Remove {
        /// Resource name
        name: String,
    },
    /// Manage all discovered resources at once
    All {
        /// Link to all detected tools
        #[arg(long)]
        all_tools: bool,
        /// Copy to tool dirs instead of symlinking
        #[arg(long)]
        copy: bool,
    },
    /// List managed resources
    List {
        /// Show duplicate resources
        #[arg(long)]
        dedup: bool,
        /// Only show content duplicates (same hash)
        #[arg(long)]
        by_hash: bool,
        /// Only show name duplicates (same name in multiple locations)
        #[arg(long)]
        by_name: bool,
    },
    /// Link a resource to a tool
    Link {
        /// Resource name
        name: String,
        /// Tool slug
        tool: String,
    },
    /// Unlink a resource from a tool
    Unlink {
        /// Resource name
        name: String,
        /// Tool slug
        tool: String,
    },
    /// Validate a SKILL.md or AGENT.md file
    Validate {
        /// Path to resource directory or markdown file (default: current dir)
        path: Option<String>,
    },
    /// Create a new resource from template
    Create {
        /// Resource name
        name: Option<String>,
        /// Resource kind
        #[arg(long, value_parser = ["skill", "agent", "session", "memory", "project-config", "llms-txt"])]
        kind: Option<String>,
    },
    /// Update managed resources
    Update {
        /// Resource name (all if omitted)
        name: Option<String>,
    },
    /// Verify integrity of managed resources (checksum validation)
    Verify {
        /// Accept current state and update hashes
        #[arg(long)]
        accept: bool,
        /// Accept only this specific resource
        #[arg(long)]
        name: Option<String>,
    },
    /// List memories across Claude Code projects
    Memory {
        /// Filter by project name or path
        #[arg(long)]
        project: Option<String>,
        /// Filter by memory type (user, feedback, project, reference)
        #[arg(long = "type")]
        mem_type: Option<String>,
    },
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
