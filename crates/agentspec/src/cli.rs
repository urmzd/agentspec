use clap::{Parser, Subcommand};

#[derive(Clone, Copy, Debug, PartialEq, clap::ValueEnum)]
pub enum OutputFormat {
    Human,
    Json,
}

#[derive(Clone, Copy, Debug, PartialEq, clap::ValueEnum)]
pub enum FleetBackend {
    Auto,
    Store,
    Tmux,
}

#[derive(Clone, Copy, Debug, PartialEq, clap::ValueEnum)]
pub enum SessionContextMode {
    Brief,
    Full,
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
    /// Manage multi-agent fleets
    Fleet {
        /// Fleet backend (auto prefers tmux when available, otherwise store)
        #[arg(long, value_enum, default_value = "auto")]
        backend: FleetBackend,
        #[command(subcommand)]
        action: FleetAction,
    },
    /// Manage git worktrees under <repo>/.worktrees/
    Worktree {
        #[command(subcommand)]
        action: WorktreeAction,
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
        /// Project name, path, or store key
        project: String,
    },
    /// Remove a project's synced copy from ~/.agents/ (originals untouched)
    Remove {
        /// Project name, path, or store key
        project: String,
    },
    /// Show synced/desynced/discovered project status
    Status {
        /// Show detailed status for a specific project (name, path, or store key)
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
pub enum FleetAction {
    /// Check tmux, agent CLIs, agentspec, and notifier availability
    Doctor,
    /// Survey active tmux panes before adopting or spawning agents
    Survey {
        /// Optional tmux session to inspect
        session: Option<String>,
    },
    /// Create or adopt a fleet
    Start {
        /// Fleet/session name
        fleet: String,
    },
    /// Tag an existing pane as a fleet agent
    Adopt {
        /// Fleet/session name
        fleet: String,
        /// Fleet pane id, such as %7 or store:fleet:agent-1
        pane: String,
        /// Agent display name
        #[arg(long)]
        name: Option<String>,
        /// Agent tool name
        #[arg(long)]
        tool: Option<String>,
    },
    /// Add a workstream window to a fleet
    Group {
        /// Fleet/session name
        fleet: String,
        /// Window name
        name: String,
    },
    /// Launch an agent CLI in a fleet window
    Spawn {
        /// Fleet/session name
        fleet: String,
        /// Window id or name
        window: String,
        /// Tool to launch: auto, claude, codex, copilot, or agy
        tool: String,
        /// Agent display name
        #[arg(long)]
        name: Option<String>,
        /// Working directory for the agent CLI
        #[arg(long, conflicts_with = "worktree")]
        dir: Option<String>,
        /// Create or reuse <repo>/.worktrees/<name> and launch the agent there
        #[arg(long)]
        worktree: Option<String>,
        /// Repository path for --worktree (default: current directory)
        #[arg(long)]
        repo: Option<String>,
        /// Branch to create for --worktree (default: worktree-<name>)
        #[arg(long)]
        branch: Option<String>,
        /// Base ref for --worktree (default: origin/HEAD when set, otherwise HEAD)
        #[arg(long)]
        base: Option<String>,
    },
    /// Send a message to a fleet pane
    Send {
        /// Fleet pane id
        pane: String,
        /// Message text
        #[arg(required = true, trailing_var_arg = true)]
        text: Vec<String>,
    },
    /// Capture recent output from a pane
    Capture {
        /// Fleet pane id
        pane: String,
        /// Number of lines to capture
        lines: Option<usize>,
    },
    /// List agents in a fleet with inferred state
    List {
        /// Fleet/session name
        fleet: String,
    },
    /// Classify one fleet pane
    State {
        /// Fleet/session name
        fleet: String,
        /// Tmux pane id
        pane: String,
    },
    /// Record a fleet agent state transition
    Mark {
        /// Fleet/session name
        fleet: String,
        /// Fleet pane id
        pane: String,
        /// New agent state
        #[arg(value_parser = ["running", "idle", "needs-permission", "error", "stuck", "done", "relayed"])]
        state: String,
        /// Optional note to record with the transition
        #[arg(long)]
        note: Option<String>,
    },
    /// Ingest a guardian state-transition contract line
    Event {
        /// Fleet/session name
        fleet: String,
        /// Guardian line: GUARDIAN[<pane>]: <state> - <summary> - <action>
        #[arg(required = true, trailing_var_arg = true)]
        line: Vec<String>,
    },
    /// Alert the user about a fleet event
    Ping {
        /// Fleet/session name
        fleet: String,
        /// Message text
        message: String,
        /// Optional pane id to target
        pane: Option<String>,
    },
    /// Start or report the fleet dashboard
    Dashboard {
        /// Fleet/session name
        fleet: String,
    },
    /// Print the tmux attach command for a fleet
    Attach {
        /// Fleet/session name
        fleet: String,
    },
    /// Tear down a fleet tmux session
    Kill {
        /// Fleet/session name
        fleet: String,
    },
}

#[derive(Subcommand)]
pub enum WorktreeAction {
    /// List git worktrees for a repository
    List {
        /// Repository path (default: current directory)
        repo: Option<String>,
    },
    /// Create a worktree under <repo>/.worktrees/<name>
    Create {
        /// Worktree slug and default branch suffix
        name: String,
        /// Repository path (default: current directory)
        #[arg(long)]
        repo: Option<String>,
        /// Branch to create (default: worktree-<name>)
        #[arg(long)]
        branch: Option<String>,
        /// Base ref (default: origin/HEAD when set, otherwise HEAD)
        #[arg(long)]
        base: Option<String>,
    },
    /// Remove a worktree under <repo>/.worktrees/
    Remove {
        /// Worktree name or path
        target: String,
        /// Repository path (default: current directory)
        #[arg(long)]
        repo: Option<String>,
        /// Pass --force to git worktree remove
        #[arg(long)]
        force: bool,
        /// Delete the checked-out branch after removing the worktree
        #[arg(long)]
        delete_branch: bool,
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
    /// Show the policy for routing or syncing context between sessions and fleet panes
    Policy,
    /// List sessions for a source
    List {
        /// Source to list (claude, codex, copilot, gemini)
        source: String,
    },
    /// Fuzzy-find a session across all sources
    Find,
    /// Correlate active fleet panes with known session transcripts
    Active {
        /// Show only one fleet pane
        #[arg(long)]
        pane: Option<String>,
    },
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
        /// Context policy: brief excludes system/developer/tool output; full stages rendered transcript
        #[arg(long, value_enum, default_value = "brief")]
        context: SessionContextMode,
        /// Add an operator note to the staged handoff
        #[arg(long)]
        note: Option<String>,
    },
    /// Import a markdown handoff into the canonical store for a target tool
    Import {
        /// Target tool (claude, codex, copilot, gemini)
        target: String,
        /// Path to the markdown handoff file
        file: String,
    },
    /// Route allowed session context into a fleet pane
    Route {
        /// Source tool (claude, codex, copilot, gemini)
        source: String,
        /// Target fleet pane id
        pane: String,
        /// Session ID (omit if using --last)
        id: Option<String>,
        /// Use the most recent source session
        #[arg(long)]
        last: bool,
        /// Fleet backend used to deliver the context
        #[arg(long, value_enum, default_value = "auto")]
        backend: FleetBackend,
        /// Context policy: brief excludes system/developer/tool output; full sends rendered transcript
        #[arg(long, value_enum, default_value = "brief")]
        context: SessionContextMode,
        /// Add an operator note to the routed context
        #[arg(long)]
        note: Option<String>,
        /// Render the routed context without sending it to the fleet pane
        #[arg(long)]
        dry_run: bool,
    },
    /// Route the best-matched active session context into a fleet pane
    RouteActive {
        /// Target fleet pane id
        pane: String,
        /// Fleet backend used to deliver the context
        #[arg(long, value_enum, default_value = "auto")]
        backend: FleetBackend,
        /// Context policy: brief excludes system/developer/tool output; full sends rendered transcript
        #[arg(long, value_enum, default_value = "brief")]
        context: SessionContextMode,
        /// Add an operator note to the routed context
        #[arg(long)]
        note: Option<String>,
        /// Render the routed context without sending it to the fleet pane
        #[arg(long)]
        dry_run: bool,
    },
    /// Route matched active session context into every matched pane in a fleet
    RouteFleet {
        /// Fleet/session name
        fleet: String,
        /// Override delivery backend for all panes; auto uses each matched pane backend
        #[arg(long, value_enum, default_value = "auto")]
        backend: FleetBackend,
        /// Context policy: brief excludes system/developer/tool output; full sends rendered transcript
        #[arg(long, value_enum, default_value = "brief")]
        context: SessionContextMode,
        /// Add an operator note to each routed context
        #[arg(long)]
        note: Option<String>,
        /// Render route previews without sending them to fleet panes
        #[arg(long)]
        dry_run: bool,
    },
}
