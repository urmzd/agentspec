<p align="center">
  <h1 align="center">agentspec</h1>
  <p align="center">
    Universal agent skill and sub-agent manager with TUI.
    <br /><br />
    <a href="#installation">Install</a>
    &middot;
    <a href="https://github.com/urmzd/agentspec/issues">Report Bug</a>
    &middot;
    <a href="https://crates.io/crates/agentspec">Crates.io</a>
  </p>
</p>

<p align="center">
  <a href="https://github.com/urmzd/agentspec/actions/workflows/ci.yml"><img src="https://github.com/urmzd/agentspec/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://crates.io/crates/agentspec"><img src="https://img.shields.io/crates/v/agentspec.svg" alt="crates.io"></a>
</p>

## Showcase

<table align="center">
  <tr>
    <td align="center">
      <img src="showcase/tui-demo.gif" alt="agentspec TUI demo" width="400" />
      <br />
      <sub><b>Interactive TUI</b></sub>
    </td>
    <td align="center">
      <img src="showcase/tui.png" alt="agentspec TUI screenshot" width="400" />
      <br />
      <sub><b>Tabbed Views</b></sub>
    </td>
  </tr>
</table>

## Features

- **Unified resource management.** Add, remove, link, validate, and create skills, agents, memories, project configs, instruction files, and llms-txt across tools.
- **Discovery & sync.** Auto-discover resources across your filesystem, adopt them, and link to all tools in one command.
- **Integrity verification.** Checksum-based verification to detect modified or corrupted resources.
- **Sessions.** List, fuzzy-find, and export AI coding sessions (Claude, Codex) as markdown.
- **Memory browser.** Browse Claude Code memories across projects, filter by type.
- **Project sync.** Sync project-level instruction files (AGENTS.md, CLAUDE.md, llms.txt) into a shared store.
- **Prune.** Remove broken resources and stale symlinks in one pass.
- **Deduplication.** Find duplicate resources by content hash or name.
- **TUI.** Interactive terminal UI with tabbed views for skills, agents, tools, sessions, memories, and configs.
- **IR layer.** Canonical representation with vendor adapters (agentskills, Claude, Gemini, Copilot).

### Supported tools

Claude Code, Cline, Windsurf, OpenHands, Gemini CLI, GitHub Copilot, Amp, Cursor, Codex, OpenCode, Kimi CLI

## Installation

### Script

```sh
curl -fsSL https://raw.githubusercontent.com/urmzd/agentspec/main/install.sh | sh
```

### Cargo

```sh
cargo install agentspec
```

### From source

```sh
git clone https://github.com/urmzd/agentspec
cd agentspec
cargo build --release
```

## Quick Start

```sh
# Launch the interactive TUI
agentspec

# Discover all resources and link to every detected tool
agentspec sync --adopt

# Add a skill from GitHub and link to all tools
agentspec manage add owner/repo --all-tools
```

## Usage

```
agentspec                                    # Launch interactive TUI

# Status & sync
agentspec status                             # Show managed and unmanaged resources
agentspec status --fast                      # Skip broad filesystem scan
agentspec sync                               # Discover, adopt, link, and verify
agentspec sync --adopt                       # Auto-adopt all discovered resources

# Resource management
agentspec manage add <source>                # Add resource (path, git URL, owner/repo)
agentspec manage add <source> --all-tools    # Add and link to all detected tools
agentspec manage add <source> --kind agent   # Override auto-detected kind
agentspec manage remove <name>               # Remove a managed resource
agentspec manage all --all-tools             # Adopt all discovered resources
agentspec manage list                        # List managed resources
agentspec manage list --dedup                # Show duplicate resources
agentspec manage link <name> <tool>          # Link a resource to a tool
agentspec manage unlink <name> <tool>        # Unlink a resource from a tool
agentspec manage validate [path]             # Validate a SKILL.md or AGENT.md
agentspec manage create --name <n> --kind skill  # Scaffold a new resource
agentspec manage update [name]               # Re-pull and refresh a resource (or all)
agentspec manage verify                      # Verify resource integrity (checksums)
agentspec manage verify --accept             # Accept current state and update hashes
agentspec manage memory                      # Browse Claude Code memories
agentspec manage memory --type feedback      # Filter memories by type

# Project config sync
agentspec project sync                       # Sync project instruction files to shared store
agentspec project status                     # Show project sync state
agentspec project desync                     # Remove project from shared store
agentspec project remove                     # Delete project config entirely

# Sessions
agentspec session find                       # Fuzzy-find a session across sources
agentspec session list claude                # List sessions for a source
agentspec session export claude --last       # Export most recent session
agentspec session export claude <id>         # Export a specific session
agentspec session export claude --last -o f.md  # Write export to file

# Cleanup
agentspec prune                              # Remove broken resources and stale symlinks
agentspec prune --yes                        # Skip confirmation prompt

# Global flags
agentspec manage list --json                 # Machine-readable JSON output
```

## Configuration

### Storage

```
~/.agents/skills/<name>/SKILL.md    Shared skill store
~/.agents/agents/<name>.md          Shared agent store
~/.config/agentspec/config.yml      Inventory and discovery cache
```

### Discovery

`agentspec status` and `agentspec sync` scan two places:

- **Tool directories.** Known paths for each detected tool (e.g. `~/.claude/skills/`, `~/.gemini/agents/`).
- **Broad filesystem walk.** Walks from `$HOME` up to 6 levels deep, looking for `SKILL.md`, agent markdown files, `AGENTS.md`, `CLAUDE.md`, and `llms.txt`. Skipped with `--fast`.

Discovered resources appear as "unmanaged" in `agentspec status`. Use `sync --adopt` or `manage add <name>` to adopt them into the shared store.

### Linking

`manage link` creates symlinks from tool directories (e.g. `~/.claude/skills/foo`) pointing into the shared store (`~/.agents/skills/foo`). Use `--copy` to copy instead of symlinking, and `--all-tools` to link to every detected tool at once.

## Roadmap

See [ROADMAP.md](ROADMAP.md) for planned features, including cross-tool session sync, memory sync, MCP server management, and more.

## License

[Apache-2.0](LICENSE)
