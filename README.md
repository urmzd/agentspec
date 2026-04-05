<div align="center">

# agentspec

Universal agent skill and sub-agent manager with TUI.

[![CI](https://github.com/urmzd/agentspec/actions/workflows/ci.yml/badge.svg)](https://github.com/urmzd/agentspec/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/agentspec.svg)](https://crates.io/crates/agentspec)

<br>
<a href="#usage">
  <img src="showcase/tui.gif" alt="agentspec TUI" width="600">
</a>
<br>

[Install](#installation) | [Usage](#usage) | [Architecture](#architecture)

</div>

## Overview

`agentspec` manages [agentskills.io](https://agentskills.io) skills and sub-agent definitions across all your AI coding tools from a single CLI. It uses an **IR (intermediate representation)** layer so vendor-specific formats (Claude Code, Gemini CLI, etc.) are translated to a canonical form, eliminating vendor lock-in.

### Supported tools

Claude Code, Cline, Windsurf, OpenHands, Gemini CLI, GitHub Copilot, Amp, Cursor, Codex, OpenCode, Kimi CLI

## Features

- **Skills & Agents** — install, remove, link, unlink, create, validate, and update across tools
- **Sessions** — list, fuzzy-find, and export AI coding sessions (Claude, Codex) as markdown
- **Tool detection** — auto-discovers installed AI coding tools on your machine
- **Search** — find skills on GitHub directly from the CLI
- **TUI** — interactive terminal UI with tabbed views and link picker
- **IR layer** — canonical representation with vendor adapters (agentskills, Claude, Gemini)

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

## Usage

```
agentspec                                # Launch interactive TUI

# Skills
agentspec skill list                     # List installed skills with tool linkage
agentspec skill install owner/repo       # Install from GitHub
agentspec skill link <skill> <tool>      # Symlink skill to a tool
agentspec skill unlink <skill> <tool>
agentspec skill validate [path]          # Validate SKILL.md
agentspec skill create [name]            # Scaffold a new skill
agentspec skill remove <name>

# Agents
agentspec agent list                     # List sub-agents across tools
agentspec agent install owner/repo
agentspec agent link <agent> <tool>
agentspec agent validate [path]
agentspec agent create [name]
agentspec agent remove <name>

# Sessions
agentspec session find                   # Fuzzy-find a session across sources
agentspec session list claude            # List sessions for a source
agentspec session export claude          # Export most recent session
agentspec session export claude <id>     # Export a specific session
agentspec session export claude -o f.md  # Write export to file

# Tools
agentspec tool list                      # Show detected AI coding tools

# Search
agentspec search <query>                 # Search GitHub for skills

# Global flags
agentspec skill list --json              # Machine-readable JSON output
```

## Architecture

```
Vendor Formats              Canonical IR                Vendor Formats
                                                    
  SKILL.md        ──┐                          ┌──▸  SKILL.md
  (agentskills)     │    ┌──────────────┐      │    (agentskills)
                    ├───▸│   Resource   │──────┤
  Claude agent    ──┤    │  (kind,name, │      ├──▸  Claude agent
  (.md + YAML)      │    │  description,│      │    (.md + YAML)
                    │    │  tools,model,│      │
  Gemini agent    ──┘    │  body,       │      └──▸  Gemini agent
  (.md + YAML)           │  extensions) │           (.md + YAML)
                         └──────────────┘
```

Each vendor gets an **adapter** implementing `parse()`, `emit()`, and `validate()`. Adding a new vendor = one file.

### Directory layout

```
~/.agents/skills/<name>/SKILL.md     # Shared skill store
~/.agents/agents/<name>.md           # Shared agent store
~/.claude/skills/<name>  → symlink   # Per-tool symlinks
~/.gemini/agents/<name>.md → symlink
```

## License

[Apache-2.0](LICENSE)
