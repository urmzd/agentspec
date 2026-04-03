<div align="center">

# agentctl

Universal agent skill and sub-agent manager with TUI.

[![CI](https://github.com/urmzd/agent-spec/actions/workflows/ci.yml/badge.svg)](https://github.com/urmzd/agent-spec/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/agent-spec.svg)](https://crates.io/crates/agent-spec)

[Install](#installation) | [Usage](#usage) | [Architecture](#architecture)

</div>

## Overview

`agentctl` manages [agentskills.io](https://agentskills.io) skills and sub-agent definitions across all your AI coding tools from a single CLI. It uses an **IR (intermediate representation)** layer so vendor-specific formats (Claude Code, Gemini CLI, etc.) are translated to a canonical form -- no vendor lock-in.

### Supported tools

Claude Code, Cline, Windsurf, OpenHands, Gemini CLI, GitHub Copilot, Amp, Cursor, Codex, OpenCode, Kimi CLI

## Features

- **Manage skills** -- install, remove, link/unlink across tools
- **Manage agents** -- sub-agent definitions with per-vendor adapters
- **Detect tools** -- auto-discovers installed AI coding tools
- **Validate** -- check SKILL.md and agent files against their specs
- **Search** -- find skills on GitHub
- **TUI** -- interactive terminal UI with tabbed views and link picker
- **IR layer** -- canonical representation with vendor adapters (agentskills, Claude, Gemini)
- **Lock file compatible** -- reads/writes existing `.skill-lock.json` v3

## Installation

### Script

```sh
curl -fsSL https://raw.githubusercontent.com/urmzd/agent-spec/main/install.sh | sh
```

### Cargo

```sh
cargo install agent-spec
```

### From source

```sh
git clone https://github.com/urmzd/agent-spec
cd agent-spec
cargo build --release
```

## Usage

```
agentctl                              # Launch interactive TUI

# Skills
agentctl skill list                   # List installed skills with tool linkage
agentctl skill install owner/repo     # Install from GitHub
agentctl skill link <skill> <tool>    # Symlink skill to a tool
agentctl skill unlink <skill> <tool>
agentctl skill validate [path]        # Validate SKILL.md
agentctl skill create [name]          # Scaffold a new skill
agentctl skill remove <name>

# Agents
agentctl agent list                   # List sub-agents across tools
agentctl agent install owner/repo
agentctl agent link <agent> <tool>
agentctl agent validate [path]
agentctl agent create [name]
agentctl agent remove <name>

# Tools
agentctl tool list                    # Show detected AI coding tools

# Search
agentctl search <query>               # Search GitHub for skills

# JSON output
agentctl skill list --json            # Machine-readable output
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
