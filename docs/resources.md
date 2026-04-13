# Resource Conventions

Authoritative reference for all resource types agentspec manages. Each section documents the convention, file format, frontmatter schema, directory layout, and discovery strategy.

---

## Skills (agentskills.io)

- **Spec**: https://agentskills.io/specification
- **GitHub**: https://github.com/agentskills/agentskills
- **Example skills**: https://github.com/anthropics/skills

An open format for extending AI agents with specialized knowledge and workflows. A skill is a directory containing a `SKILL.md` file with YAML frontmatter and Markdown instructions.

### Directory Structure

```
{name}/
├── SKILL.md           # Required: metadata + instructions
├── scripts/           # Optional: executable code
├── references/        # Optional: documentation
└── assets/            # Optional: templates, resources
```

### Locations

| Scope | Path | Notes |
|-------|------|-------|
| Project | `.{tool}/skills/{name}/SKILL.md` | Tool-specific (`.claude/`, `.gemini/`, etc.) |
| Project | `.agents/skills/{name}/SKILL.md` | Cross-tool interop (Codex, Copilot) |
| User | `~/.{tool}/skills/{name}/SKILL.md` | Tool-specific user skills |
| User | `~/.agents/skills/{name}/SKILL.md` | Cross-tool user skills (shared store) |

### SKILL.md Frontmatter

| Field | Required | Constraints |
|-------|----------|-------------|
| `name` | Yes | 1-64 chars, lowercase alphanumeric + hyphens, no leading/trailing/consecutive hyphens, must match parent directory name |
| `description` | Yes | 1-1024 chars, what the skill does and when to use it |
| `license` | No | License name or reference to bundled license file |
| `compatibility` | No | Max 500 chars, environment requirements |
| `metadata` | No | Arbitrary key-value mapping |
| `allowed-tools` | No | Space-delimited list of pre-approved tools (experimental) |

### Claude Code Extensions

Fields beyond the agentskills.io spec:

| Field | Description |
|-------|-------------|
| `disable-model-invocation` | `true` to prevent Claude from auto-loading |
| `user-invocable` | `false` to hide from `/` menu |
| `model` | Model override |
| `effort` | Effort level: `low`, `medium`, `high`, `max` |
| `context` | `fork` to run in a subagent |
| `agent` | Which subagent type when `context: fork` |
| `hooks` | Lifecycle hooks scoped to this skill |
| `paths` | Glob patterns limiting auto-activation |
| `argument-hint` | Hint shown during autocomplete |
| `shell` | `bash` or `powershell` |

### Progressive Disclosure

1. **Catalog** (~50-100 tokens/skill): Name + description loaded at startup
2. **Instructions** (<5000 tokens): Full SKILL.md body on activation
3. **Resources** (varies): Scripts/references/assets loaded on demand

### Supported Tools

Claude Code, Gemini CLI, Codex, Cursor, VS Code (Copilot), GitHub Copilot, OpenCode, OpenHands, Goose, Roo Code, Amp, Junie, Kiro, Letta, and others.

### Discovery

Match `SKILL.md` filename during filesystem walk. Parent directory name = skill name.

---

## Agents / Subagents

Specialized AI assistants delegated to by the main agent. Each runs in its own context window with a custom system prompt and tool restrictions.

### Convention by Tool

| Tool | Format | File Pattern |
|------|--------|-------------|
| Claude Code | YAML frontmatter + Markdown | `agents/{name}.md` |
| Gemini CLI | YAML frontmatter + Markdown | `agents/{name}.md` |
| Codex | TOML | `agents/{name}.toml` |

### Locations

| Scope | Claude Code | Gemini CLI | Codex |
|-------|-------------|------------|-------|
| Project | `.claude/agents/{name}.md` | `.gemini/agents/{name}.md` | `.codex/agents/{name}.toml` |
| User | `~/.claude/agents/{name}.md` | `~/.gemini/agents/{name}.md` | `~/.codex/agents/{name}.toml` |
| Shared | `~/.agents/agents/{name}.md` | `~/.agents/agents/{name}.md` | — |

### Claude Code Frontmatter

| Field | Required | Description |
|-------|----------|-------------|
| `name` | Yes | Unique identifier, lowercase letters and hyphens |
| `description` | Yes | When Claude should delegate to this subagent |
| `tools` | No | Comma-separated string or YAML list. Inherits all if omitted |
| `disallowedTools` | No | Tools to deny, removed from inherited/specified list |
| `model` | No | `sonnet`, `opus`, `haiku`, full model ID, or `inherit` (default) |
| `permissionMode` | No | `default`, `acceptEdits`, `auto`, `dontAsk`, `bypassPermissions`, `plan` |
| `maxTurns` | No | Maximum agentic turns before subagent stops |
| `skills` | No | Skills to preload into subagent context at startup |
| `mcpServers` | No | MCP servers scoped to this subagent (inline or reference) |
| `hooks` | No | Lifecycle hooks scoped to this subagent |
| `memory` | No | Persistent memory scope: `user`, `project`, `local` |
| `background` | No | `true` to always run as background task |
| `effort` | No | `low`, `medium`, `high`, `max` |
| `isolation` | No | `worktree` for git worktree isolation |
| `color` | No | `red`, `blue`, `green`, `yellow`, `purple`, `orange`, `pink`, `cyan` |
| `initialPrompt` | No | Auto-submitted first user turn when running as main agent via `--agent` |

Built-in agents: `Explore`, `Plan`, `general-purpose`, `statusline-setup`, `claude-code-guide`.

### Gemini CLI Frontmatter

| Field | Required | Default | Description |
|-------|----------|---------|-------------|
| `name` | Yes | — | Lowercase letters, numbers, hyphens, underscores |
| `description` | Yes | — | When to invoke this subagent |
| `kind` | No | `local` | `local` or `remote` |
| `tools` | No | inherited | Tool names, supports wildcards (`*`, `mcp_*`, `mcp_server_*`) |
| `mcpServers` | No | — | Inline MCP server configs isolated to this agent |
| `model` | No | inherited | Model override |
| `temperature` | No | `1` | 0.0-2.0 |
| `max_turns` | No | `30` | Max conversation turns |
| `timeout_mins` | No | `10` | Max execution time in minutes |

### Codex Agent Format (TOML)

| Field | Required | Description |
|-------|----------|-------------|
| `name` | Yes | Agent identifier used when spawning |
| `description` | Yes | Human-facing usage guidance |
| `developer_instructions` | Yes | Core behavioral instructions |
| `nickname_candidates` | No | Display name pool (ASCII letters, digits, spaces, hyphens, underscores; unique) |
| `model` | No | Model override (inherits from parent session) |
| `model_reasoning_effort` | No | Reasoning effort override |
| `sandbox_mode` | No | Sandbox config override |
| `mcp_servers` | No | MCP server definitions |
| `skills.config` | No | Skills configuration |

Codex global settings (`[agents]` in `config.toml`):

| Field | Default | Description |
|-------|---------|-------------|
| `max_threads` | `6` | Concurrent open agent thread cap |
| `max_depth` | `1` | Spawned agent nesting depth |
| `job_max_runtime_seconds` | `1800` | Timeout per worker for CSV batch jobs |

Built-in agents: `default`, `worker`, `explorer`.

### Discovery

Scan `.md` (or `.toml` for Codex) files inside directories named `agents`. Validate by parsing frontmatter for required `name` + `description` fields.

---

## AGENTS.md (Project Config)

- **Website**: https://agents.md
- **Stewarded by**: Agentic AI Foundation (Linux Foundation)

Root-level markdown file providing project instructions to coding agents. Tool-agnostic equivalent of CLAUDE.md — described as "a README for agents."

### Location

Project root: `./AGENTS.md`

### Format

Plain markdown. No required YAML frontmatter. Common sections:

- Project overview and architecture
- Build and test commands
- Code style conventions
- Testing instructions
- Security considerations
- PR/commit message guidelines

### Hierarchical Resolution

For monorepos, nested `AGENTS.md` files take precedence based on file proximity — the closest file to the working directory wins.

### Relationship to CLAUDE.md

Claude Code reads `CLAUDE.md`, not `AGENTS.md`. To share instructions, `CLAUDE.md` can import it:

```markdown
@AGENTS.md
```

### Discovery

Filename match (`AGENTS.md`) in project roots (directories containing `.git`).

### Adoption

60k+ open-source projects. Supported by Cursor, Zed, VS Code, GitHub Copilot, and others.

---

## CLAUDE.md (Claude Code Config)

- **Docs**: https://code.claude.com/docs/en/memory

Root-level markdown file providing persistent instructions to Claude Code.

### Locations

| Scope | Location |
|-------|----------|
| Managed policy | `/Library/Application Support/ClaudeCode/CLAUDE.md` (macOS), `/etc/claude-code/CLAUDE.md` (Linux) |
| Project | `./CLAUDE.md` or `./.claude/CLAUDE.md` |
| User | `~/.claude/CLAUDE.md` |
| Local | `./CLAUDE.local.md` (gitignored) |

### Format

Plain markdown with optional features:

- `@path/to/file` imports (relative or absolute, max 5 hops deep)
- `.claude/rules/*.md` for modular path-scoped rules (with `paths:` YAML frontmatter)
- HTML comments stripped before context injection

### Discovery

Walk up directory tree from cwd, checking each level for `CLAUDE.md` and `CLAUDE.local.md`. Subdirectory files load on demand when Claude reads files in those directories.

---

## llms.txt

- **Website**: https://llmstxt.org
- **Spec**: https://llmstxt.org/llms.txt

Root-level file providing LLM-friendly project summaries. Analogous to `/robots.txt`.

### Location

Project root: `./llms.txt`

### Format (required order)

1. **H1 heading** (required) — project/site name
2. **Blockquote** (optional) — brief summary with key info
3. **Body content** (optional) — project details as paragraphs or lists
4. **H2 sections** (optional) — curated resource links as `[name](url): description`
5. **"Optional" H2** (optional) — secondary info skippable under context constraints

### Variants

| File | Purpose |
|------|---------|
| `/llms.txt` | Curated overview with key links |
| `/llms-full.txt` | Complete detailed documentation |

### Discovery

Filename match (`llms.txt`) in project roots.

---

## Instruction Files

Editor/tool-specific instruction files that live alongside project code. Unlike `CLAUDE.md` (which is Claude Code-specific) or `AGENTS.md` (which is tool-agnostic), these are per-tool configuration files each tool reads natively.

### Known Files

| File | Tool |
|------|------|
| `.cursorrules` | Cursor |
| `.clinerules` | Cline |
| `GEMINI.md` | Gemini CLI |
| `.github/copilot-instructions.md` | GitHub Copilot |
| `codex-instructions.md` | Codex |

### Location

Project root (or `.github/` for Copilot). Each file is scoped to its owning tool.

### Format

Plain markdown. No required YAML frontmatter.

### Discovery

Filename match against the known list during filesystem walk. Detected as `InstructionFile` kind with the owning tool derived from the filename.

### Project Sync

agentspec's `project sync` command copies instruction files into `~/.agents/projects/<project>/` as a shared snapshot. The project root (identified by a `.git` directory) is the canonical source.

---

## Plans

Recorded planning artifacts — structured task breakdowns generated by or for AI coding tools.

### Location

`~/.agents/plans/<name>.md`

### Format

Plain markdown. Optional YAML frontmatter.

### Discovery

Scan known plan directories. Plans are first-class resources in agentspec's inventory and appear in the **Configs** TUI tab alongside project configs and llms.txt files.

---

## Memories (Claude Code)

Auto-memory files managed by Claude Code across sessions.

### Location

`~/.claude/projects/<project>/memory/`

Project path derived from git repository root. All worktrees share one memory directory.

### Structure

```
memory/
├── MEMORY.md          # Index (first 200 lines / 25KB loaded at session start)
├── {topic}.md         # Detailed topic files (loaded on demand)
└── ...
```

### Memory File Frontmatter

| Field | Description |
|-------|-------------|
| `name` | Memory name |
| `description` | One-line description for relevance matching |
| `type` | `user`, `feedback`, `project`, `reference` |

### Discovery

Scan Claude Code project directories (`~/.claude/projects/*/memory/`) for `.md` files with YAML frontmatter.

---

## Sessions

Tool-specific session history exports.

### Sources

| Tool | Location |
|------|----------|
| Claude Code | `~/.claude/projects/*/sessions/` |
| Codex | Tool-specific paths |

### Format

JSON (tool-specific schema). Exportable as markdown via `agentspec session export`.

### Discovery

Known tool-specific paths. Sessions identified by ID and timestamp.

---

## MCP (Model Context Protocol)

- **Website**: https://modelcontextprotocol.io
- **Spec**: https://modelcontextprotocol.io/docs/learn/architecture
- **Rust SDK**: https://github.com/modelcontextprotocol/rust-sdk (`rmcp` crate)

Open protocol for connecting AI applications to external data sources, tools, and workflows.

### Configuration Files

| Tool | File | Location | Docs |
|------|------|----------|------|
| Claude Code | `.mcp.json` | Project root (shared, commit to git) | https://code.claude.com/docs/en/mcp |
| Claude Code | `~/.claude.json` | User/local scope (`mcpServers` key) | https://code.claude.com/docs/en/mcp |
| Claude Code | `~/.claude/settings.json` | Allow/denylists only (`allowedMcpServers`) | |
| Gemini CLI | `~/.gemini/settings.json` | Global (`mcpServers` key) | https://geminicli.com/docs/tools/mcp-server/ |
| Gemini CLI | `.gemini/settings.json` | Project-level | |
| Cursor | `~/.cursor/mcp.json` | Global | |
| Cursor | `.cursor/mcp.json` | Project-level | |
| VS Code | `.vscode/mcp.json` | Workspace | |

### .mcp.json Format (Project Declaration)

Projects declare their MCP servers in `.mcp.json` at the project root. agentspec discovers this during `sync` and registers servers in each tool's native config.

```json
{
  "mcpServers": {
    "server-name": {
      "command": "/path/to/server",
      "args": ["--flag", "value"],
      "env": { "API_KEY": "${API_KEY}" }
    }
  }
}
```

### Server Types

| Type | Transport | Use Case |
|------|-----------|----------|
| stdio | stdin/stdout | Local process servers |
| sse | Server-Sent Events | Remote streaming servers (deprecated in Claude Code) |
| streamable-http | HTTP POST + SSE | Remote servers (recommended) |

### Tool-Specific Differences

| Feature | Claude Code | Gemini CLI | Cursor |
|---------|-------------|------------|--------|
| Config key | `mcpServers` | `mcpServers` | `mcpServers` |
| Transport selector | explicit `type` field | implicit (key-based: `command`/`url`/`httpUrl`) | implicit |
| Env var syntax | `${VAR}`, `${VAR:-default}` | `$VAR`, `${VAR}` | `${env:VAR}` |
| Tool filtering | `allowedMcpServers` in settings.json | `includeTools`/`excludeTools` per server | — |
| HTTP transport | `type: "http"` + `url` | `httpUrl` | — |

### agentspec MCP Management

Three-layer abstraction:

1. **`.mcp.json`** at project root — our declaration spec (what servers a project exposes)
2. **`CodingTool::mcp_config_path()`** — each provider defines where its MCP config lives
3. **`agentspec sync`** — discovers `.mcp.json`, writes to each provider's native format

Commands:
- `agentspec mcp add <name> --command <cmd> --args <args>` — register globally
- `agentspec mcp remove <name>` — unregister
- `agentspec mcp list` — show registered servers
- `agentspec sync` — auto-discovers `.mcp.json` in known project roots

---

## Cross-Resource Comparison

| Resource | File Pattern | Format | Discovery Strategy |
|----------|-------------|--------|-------------------|
| Skills | `{name}/SKILL.md` | YAML + Markdown | `SKILL.md` filename match |
| Agents | `agents/{name}.md` | YAML + Markdown | `agents/` parent dir + frontmatter validation |
| Agents (Codex) | `agents/{name}.toml` | TOML | `agents/` parent dir |
| AGENTS.md | `AGENTS.md` | Markdown | Filename match in project roots |
| CLAUDE.md | `CLAUDE.md` | Markdown | Directory tree walk |
| llms.txt | `llms.txt` | Markdown | Filename match in project roots |
| Instruction files | `.cursorrules`, `GEMINI.md`, etc. | Markdown | Known filename match per tool |
| Plans | `plans/{name}.md` | Markdown | Known plan directories |
| Memories | `memory/*.md` | YAML + Markdown | Known Claude Code project paths |
| Sessions | `sessions/*.json` | JSON | Known tool-specific paths |
| MCP | `.mcp.json` | JSON | Known config file locations |

---

## Links

### Specifications
- https://agentskills.io/specification
- https://llmstxt.org
- https://agents.md
- https://modelcontextprotocol.io

### Tool Documentation
- https://code.claude.com/docs/en/skills
- https://code.claude.com/docs/en/sub-agents
- https://code.claude.com/docs/en/mcp
- https://geminicli.com/docs/core/subagents/
- https://geminicli.com/docs/cli/skills/
- https://geminicli.com/docs/tools/mcp-server/
- https://developers.openai.com/codex/skills/
- https://developers.openai.com/codex/subagents

### Repositories
- https://github.com/agentskills/agentskills
- https://github.com/anthropics/skills
- https://github.com/modelcontextprotocol
- https://github.com/modelcontextprotocol/rust-sdk
