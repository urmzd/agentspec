use clap::{Parser, Subcommand};

#[derive(Clone, Copy, Debug, PartialEq, clap::ValueEnum)]
pub enum OutputFormat {
    Human,
    Json,
}

#[derive(Parser)]
#[command(
    name = "agentspec",
    about = "Universal agent skill & sub-agent manager"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Output format
    #[arg(long, global = true, value_enum, default_value = "human")]
    pub format: OutputFormat,
}

#[derive(Subcommand)]
pub enum Command {
    /// Manage resources (skills, agents, memories)
    Manage {
        #[command(subcommand)]
        action: ManageAction,
    },
    /// Show managed and unmanaged resource inventory
    Status {
        /// Root directory for broad discovery (default: $HOME)
        #[arg(long)]
        root: Option<String>,
        /// Skip broad discovery, only scan known tool dirs
        #[arg(long)]
        fast: bool,
        /// Extra paths to scan (bypasses SKIP_DIRS)
        #[arg(long, value_delimiter = ',')]
        path: Option<Vec<String>>,
    },
    /// Discover, adopt, link, and verify all resources
    Sync {
        /// Root directory for broad discovery (default: $HOME)
        #[arg(long)]
        root: Option<String>,
        /// Skip broad scan, only check known dirs
        #[arg(long)]
        fast: bool,
        /// Auto-adopt all discovered resources without prompting
        #[arg(long)]
        adopt: bool,
        /// Extra paths to scan (bypasses SKIP_DIRS)
        #[arg(long, value_delimiter = ',')]
        path: Option<Vec<String>>,
    },
    /// Manage AI coding sessions
    Session {
        #[command(subcommand)]
        action: SessionAction,
    },
    /// Manage project sync (sync instruction files into ~/.agents/projects/)
    Project {
        #[command(subcommand)]
        action: ProjectAction,
    },
    /// Remove broken resources, stale symlinks, and orphaned entries
    Prune {
        /// Actually remove — default is dry-run (show only)
        #[arg(long, short = 'y')]
        yes: bool,
    },
    /// Manage MCP servers across AI tools
    Mcp {
        #[command(subcommand)]
        action: McpAction,
    },
    /// Import and list planning artifacts (~/.agents/plans/)
    Plans {
        #[command(subcommand)]
        action: PlansAction,
    },
    /// Sync a portable permission profile into each tool's allowlist
    Permissions {
        #[command(subcommand)]
        action: PermissionsAction,
    },
    /// Inventory and export installed plugins
    Plugins {
        #[command(subcommand)]
        action: PluginsAction,
    },
    /// Manage portable lifecycle hooks (~/.agents/hooks/)
    Hooks {
        #[command(subcommand)]
        action: HooksAction,
    },
    /// Update agentspec to the latest release
    Update,
    /// Print version
    Version,
}

#[derive(Subcommand)]
pub enum ManageAction {
    /// Add a resource (local path, git URL, owner/repo, or discovered name)
    Add {
        /// Source: local path, git URL, owner/repo, or name of a discovered resource
        source: String,
        /// Override auto-detected kind
        #[arg(long, value_parser = ["skill", "agent", "project-config", "instruction-file", "llms-txt", "memory", "session", "plan"])]
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
        #[arg(long, value_parser = ["skill", "agent", "project-config", "instruction-file", "llms-txt", "memory", "session", "plan"])]
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
    /// List or sync memories across Claude Code projects
    Memory {
        /// Filter by project name or path
        #[arg(long)]
        project: Option<String>,
        /// Filter by memory type (user, feedback, project, reference)
        #[arg(long = "type")]
        mem_type: Option<String>,
        /// Pull tool memories into the canonical store (~/.agents/memories/)
        #[arg(long, conflicts_with = "push")]
        pull: bool,
        /// Push canonical memories back into tool stores
        #[arg(long)]
        push: bool,
    },
}

#[derive(Subcommand)]
pub enum ProjectAction {
    /// Sync a project's instruction files into ~/.agents/projects/
    Sync {
        /// Project name or path (syncs all if omitted)
        project: Option<String>,
    },
    /// Stop auto-sync for a project (copy stays but goes stale)
    Desync {
        /// Project name
        project: String,
    },
    /// Remove a project's synced copy from ~/.agents/ (originals untouched)
    Remove {
        /// Project name
        project: String,
    },
    /// Show synced/desynced/discovered project status
    Status {
        /// Show detailed status for a specific project
        #[arg(long)]
        project: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum McpAction {
    /// Register an MCP server in the canonical store and AI tool configs
    Add {
        /// Server name (e.g. "sr")
        name: String,
        /// Command to run a stdio server (e.g. "sr")
        #[arg(long)]
        command: Option<String>,
        /// Arguments for the command (e.g. "mcp serve")
        #[arg(long, value_delimiter = ' ')]
        args: Vec<String>,
        /// Environment variables as KEY=VALUE (repeatable)
        #[arg(long = "env", value_name = "KEY=VAL")]
        env: Vec<String>,
        /// URL for an http/sse server
        #[arg(long)]
        url: Option<String>,
        /// Transport type: stdio, http, or sse
        #[arg(long = "type", value_parser = ["stdio", "http", "sse"])]
        server_type: Option<String>,
        /// Register only in a specific tool (claude-code, gemini-cli, cursor)
        #[arg(long)]
        tool: Option<String>,
    },
    /// Remove an MCP server from AI tool configs (and the store)
    Remove {
        /// Server name
        name: String,
        /// Remove only from a specific tool (claude-code, gemini-cli, cursor)
        #[arg(long)]
        tool: Option<String>,
        /// Also delete from the canonical store (~/.agents/mcp/<name>.json)
        #[arg(long)]
        purge: bool,
    },
    /// List MCP servers (canonical store + per-tool registrations)
    List,
    /// Inject a stored server into tool config(s)
    Link {
        /// Name of a server in the canonical store
        name: String,
        /// Target a specific tool
        #[arg(long)]
        tool: Option<String>,
        /// Link to all installed MCP-capable tools
        #[arg(long)]
        all_tools: bool,
    },
    /// Link all canonical servers to all installed MCP-capable tools
    Sync,
}

#[derive(Subcommand)]
pub enum HooksAction {
    /// Add a hook script into the canonical store
    Add {
        /// Path to the hook script
        path: String,
    },
    /// List hooks (canonical store + per-tool)
    List,
    /// Link a stored hook into tool hook directories
    Link {
        /// Hook name (filename in the store)
        name: String,
        /// Target a specific tool (claude-code)
        #[arg(long)]
        tool: Option<String>,
        /// Link to all hooks-capable tools
        #[arg(long)]
        all_tools: bool,
    },
}

#[derive(Subcommand)]
pub enum PluginsAction {
    /// List installed plugins (Claude Code)
    List,
    /// Export a portable plugin manifest (~/.agents/plugins.yml or a path)
    Export {
        /// Write to a specific file instead of the default manifest
        #[arg(short, long)]
        output: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum PermissionsAction {
    /// Scaffold ~/.agents/permissions.yml with example rules
    Init {
        /// Overwrite an existing profile
        #[arg(long)]
        force: bool,
    },
    /// Translate the profile into each tool's native allowlist
    Sync {
        /// Sync only a specific tool (claude-code, gemini-cli)
        #[arg(long)]
        tool: Option<String>,
        /// Show what would change without writing
        #[arg(long)]
        dry_run: bool,
    },
    /// Show the profile and per-tool rendered allowlists
    Show {
        /// Show only a specific tool
        #[arg(long)]
        tool: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum PlansAction {
    /// Import planning artifacts into the canonical store (~/.agents/plans/)
    Import {
        /// Source to import from (currently: gemini)
        #[arg(default_value = "gemini")]
        source: String,
    },
    /// List plans in the canonical store
    List,
}

#[derive(Subcommand)]
pub enum SessionAction {
    /// List sessions for a source
    List {
        /// Source to list (claude, codex, copilot, gemini)
        source: String,
    },
    /// Fuzzy-find a session across all sources
    Find,
    /// Export a session as markdown
    Export {
        /// Source (claude, codex, copilot, gemini)
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
    /// Translate a session from one tool into a portable handoff staged for another
    Sync {
        /// Source tool (claude, codex, copilot, gemini)
        source: String,
        /// Target tool the handoff is staged for
        target: String,
        /// Session ID (omit if using --last)
        id: Option<String>,
        /// Use the most recent source session
        #[arg(long)]
        last: bool,
    },
    /// Import a markdown handoff into the canonical store for a target tool
    Import {
        /// Target tool (claude, codex, copilot, gemini)
        target: String,
        /// Path to the markdown handoff file
        file: String,
    },
}
