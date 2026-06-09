# Resource Conventions

Authoritative reference for all resource types agentspec manages. Each section documents the convention, file format, frontmatter schema, directory layout, and discovery strategy.

---

## Skills (agentskills.io)

- **Spec**: https://agentskills.io/specification
- **GitHub**: https://github.com/agentskills/agentskills
- **Example skills**: https://github.com/anthropics/skills

An open format for extending AI agents with specialized knowledge and workflows. A skill is a directory containing a `SKILL.md` file with YAML frontmatter and Markdown instructions.

### Directory Structure

A skill directory `{name}/` contains:

- `SKILL.md` (required): metadata + instructions
- `scripts/` (optional): executable code
- `references/` (optional): documentation
- `assets/` (optional): templates, resources

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

agentspec implements adapters for **11 tools**, all of which expose both a skills and an agents directory: Claude Code (`claude-code`), Cline (`cline`), Windsurf (`windsurf`), OpenHands (`openhands`), Gemini CLI (`gemini-cli`), GitHub Copilot (`github-copilot`), Amp (`amp`), Cursor (`cursor`), Codex (`codex`), OpenCode (`opencode`), and Kimi CLI (`kimi-cli`).

Other agents in the ecosystem (Goose, Roo Code, Junie, Kiro, Letta, Zed, etc.) adopt the agentskills.io format upstream but do **not** yet have a dedicated agentspec adapter; treat them as aspirational/spec reference, not as tools agentspec writes to today.

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

> **Spec reference / not yet implemented.** agentspec's IR and adapter layer do **not** currently parse or emit Codex `.toml` agents; agent discovery is `.md`-only (YAML frontmatter + Markdown). The schema below is documented for reference; round-tripping Codex TOML agents through agentspec is a future addition.

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

Scan `.md` files inside directories named `agents`. Validate by parsing frontmatter for required `name` + `description` fields. (Codex `.toml` agents are not yet discovered; see the note above.)

---

## AGENTS.md (Project Config)

- **Website**: https://agents.md
- **Stewarded by**: Agentic AI Foundation (Linux Foundation)

Root-level markdown file providing project instructions to coding agents. Tool-agnostic equivalent of CLAUDE.md, described as "a README for agents."

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

For monorepos, nested `AGENTS.md` files take precedence based on file proximity: the closest file to the working directory wins.

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

1. **H1 heading** (required): project/site name
2. **Blockquote** (optional): brief summary with key info
3. **Body content** (optional): project details as paragraphs or lists
4. **H2 sections** (optional): curated resource links as `[name](url): description`
5. **"Optional" H2** (optional): secondary info skippable under context constraints

### Variants

| File | Purpose |
|------|---------|
| `/llms.txt` | Curated overview with key links |
| `/llms-full.txt` | Complete detailed documentation |

### Discovery

Filename match (`llms.txt`) in project roots.

---

## Instruction Files

Editor/tool-specific instruction files that live alongside project code, each read natively by its owning tool. `AGENTS.md` (tool-agnostic project config) and `llms.txt` are tracked separately as their own kinds.

### Known Files

This table mirrors `PROJECT_FILES` in `crates/agentspec/src/project_files.rs`.

| Project file | Tool | Global file |
|--------------|------|-------------|
| `CLAUDE.md` | Claude Code (see its own section above) | `~/.claude/CLAUDE.md` |
| `GEMINI.md` | Gemini CLI | `~/.gemini/GEMINI.md` |
| `.github/copilot-instructions.md` | GitHub Copilot | — |
| `codex.md` | Codex | `~/.codex/instructions.md` |
| `.cursorrules` | Cursor | — |
| `.cursor/rules/` (directory) | Cursor | — |
| `.clinerules` | Cline | — |
| `.windsurfrules` | Windsurf | — |

### Location

Project root (or `.github/` for Copilot, `.cursor/` for Cursor rules directories). Each file is scoped to its owning tool.

### Format

Plain markdown. No required YAML frontmatter.

### Discovery

Filename match against the known list during filesystem walk. Detected as `InstructionFile` kind with the owning tool derived from the filename.

### Project Sync

agentspec's `project sync` command copies instruction files into `~/.agents/projects/<key>/` as a shared snapshot, where `<key>` is the project's full path encoded Claude-style (`/Users/u/work/app` becomes `-Users-u-work-app`), so two projects sharing a folder name never collide. The project root (identified by a `.git` directory) is the canonical source. `project desync`/`project remove` accept the key, the full path, or a directory basename when it is unambiguous.

---

## Plans

Recorded planning artifacts: structured task breakdowns generated by or for AI coding tools.

### Location

`~/.agents/plans/<name>.md`

### Format

Plain markdown with YAML frontmatter (see the import schema below).

### Importing (Gemini antigravity)

`agentspec plans import [gemini]` pulls Gemini CLI **antigravity** planning artifacts from `~/.gemini/antigravity/brain/<session>/` into the canonical store. For each session it reads the `task`, `implementation_plan`, and `walkthrough` artifacts, preferring the `.md.resolved` view (with `file://` links/placeholders expanded) and falling back to the raw `.md`, and merges metadata from the sibling `.md.metadata.json`. Each artifact lands at `~/.agents/plans/<artifact>-<short-uuid>.md` (artifact base with underscores swapped for hyphens, plus the first 8 chars of the session UUID).

`agentspec plans list` lists the plans in the canonical store.

#### Imported Frontmatter Schema

| Field | Description |
|-------|-------------|
| `name` | `<artifact>-<short-uuid>` (e.g. `implementation-plan-1a2b3c4d`) |
| `description` | Artifact summary from metadata, or `Imported <artifact_type> plan` fallback |
| `source` | Always `gemini-antigravity` |
| `session` | Full antigravity session UUID |
| `artifact_type` | `task`, `implementation_plan`, or `walkthrough` |
| `updated_at` | Timestamp from metadata (omitted if absent) |

### Discovery

Plans are **discovered** by scanning `~/.agents/plans/*.md`. They are first-class resources (`ResourceKind::Plan`) in agentspec's inventory; `status` and `manage list` surface them, showing any untracked plan as unmanaged.

> Plans do **not** appear in the **Configs** TUI tab. That tab shows per-project file-readiness indicators for project configs / instruction files / llms.txt, not plans.

---

## Memories (Claude Code)

Auto-memory files managed by Claude Code across sessions.

### Location

`~/.claude/projects/<project>/memory/`

Project path derived from git repository root. All worktrees share one memory directory.

### Structure

- `memory/MEMORY.md`: index (first 200 lines / 25KB loaded at session start)
- `memory/{topic}.md`: detailed topic files (loaded on demand)

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

Four read sources are supported: `claude`, `codex`, `copilot`, `gemini`.

| Tool | Slug | Location |
|------|------|----------|
| Claude Code | `claude` | `~/.claude/projects/*/` (JSONL transcripts) |
| Codex | `codex` | Codex session store |
| GitHub Copilot | `copilot` | `~/.copilot/` (`events.jsonl` + `session-store.db`) |
| Gemini CLI | `gemini` | Gemini CLI session store |

### Format

Native tool schemas (JSONL / SQLite, tool-specific). Exportable as portable markdown via `agentspec session export <source> [id] [--last]`.

### Copilot Enrichment

Copilot exports are enriched from `~/.copilot/session-store.db` (SQLite, via `rusqlite`). On top of the message stream in `events.jsonl`, the export pulls the session summary, repository (as project), branch, files touched (`## Files Touched`), checkpoints (`## Checkpoints` with overview / work-done / technical-details / next-steps), and references (`## References`) from the `sessions`, `session_files`, `checkpoints`, and `session_refs` tables. A missing DB is a graceful no-op.

### Cross-Tool Handoffs

Native session stores are append-only / tool-internal, so agentspec does not fabricate native session files. Instead it stages a portable markdown handoff keyed by the **target** tool under `~/.agents/sessions/<target>/`:

- `agentspec session sync <source> <target> [<id>] [--last]`: load a session from the source tool (reusing the read adapter + IR), render a portable markdown handoff, and stage it at `~/.agents/sessions/<target>/<id>.md`.
- `agentspec session import <target> <file>`: stage an external markdown handoff at `~/.agents/sessions/<target>/<stem>.md`.

### Discovery

Known tool-specific paths. Sessions identified by ID and timestamp.

---

## MCP (Model Context Protocol)

- **Website**: https://modelcontextprotocol.io
- **Spec**: https://modelcontextprotocol.io/docs/learn/architecture
- **Rust SDK**: https://github.com/modelcontextprotocol/rust-sdk (`rmcp` crate)

Open protocol for connecting AI applications to external data sources, tools, and workflows.

### Configuration Files

Only three tools expose an `mcp_config_path()` and are therefore **agentspec write targets**: Claude Code (`.claude/settings.json`), Gemini CLI (`.gemini/settings.json`), and Cursor (`.cursor/mcp.json`). The remaining rows below (`~/.claude.json`, `.vscode/mcp.json`, etc.) are **spec reference** for where the MCP ecosystem stores config; agentspec does not write them.

| Tool | File | Location | agentspec write target | Docs |
|------|------|----------|------------------------|------|
| Claude Code | `.claude/settings.json` | User/global (`mcpServers` key) | **Yes** | https://code.claude.com/docs/en/mcp |
| Gemini CLI | `.gemini/settings.json` | User/global + project (`mcpServers` key) | **Yes** | https://geminicli.com/docs/tools/mcp-server/ |
| Cursor | `.cursor/mcp.json` | User/global + project (`mcpServers` key) | **Yes** | |
| Claude Code | `.mcp.json` | Project root (shared, commit to git) | reference (discovered by `sync`) | https://code.claude.com/docs/en/mcp |
| Claude Code | `~/.claude.json` | User/local scope (`mcpServers` key) | reference only | https://code.claude.com/docs/en/mcp |
| VS Code | `.vscode/mcp.json` | Workspace | reference only | |

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

agentspec keeps a **canonical store** of MCP servers at `~/.agents/mcp/<name>.json` (one file per server, in the same shape tools expect) and injects each into the native configs of MCP-capable installed tools (Claude Code, Gemini CLI, Cursor).

Layers:

1. **Canonical store**: `~/.agents/mcp/<name>.json`, the portable source of truth.
2. **`.mcp.json`** at a project root: a project's own declaration; `agentspec sync` still auto-discovers these.
3. **`CodingTool::mcp_config_path()`**: each MCP-capable provider defines where its native config lives.

Commands:

- `agentspec mcp add <name> [--command <cmd>] [--args "a b"] [--env KEY=VAL ...] [--url <url>] [--type stdio|http|sse] [--tool <slug>]`: register a server. Requires `--command` (stdio) **xor** `--url` (remote http/sse). `--args` is space-delimited; `--env` is repeatable `KEY=VALUE` pairs (both **are** supported). Writes `~/.agents/mcp/<name>.json` **and** injects into every MCP-capable installed tool, or just `--tool` if given.
- `agentspec mcp remove <name> [--tool <slug>] [--purge]`: remove from tool configs. Also deletes the canonical store file when no `--tool` is given, or when `--purge` is set.
- `agentspec mcp list`: show the canonical store **and** each tool's registered servers.
- `agentspec mcp link <name> [--tool <slug>] [--all-tools]`: inject a stored server into tool config(s).
- `agentspec mcp sync`: link every canonical server into all MCP-capable installed tools.
- `agentspec sync`: additionally auto-discovers `.mcp.json` in known project roots.

---

## Permissions

A portable allow/deny permission profile that translates into each tool's native allowlist.

### Location

`~/.agents/permissions.yml`

### Format

```yaml
allow:
  - kind: shell
    cmd: cat
    pattern: "*"
  - kind: shell
    cmd: ls
  - kind: file_read
    pattern: "**"
deny: []
```

Each rule has a `kind`, plus optional `cmd` and `pattern` fields:

| `kind` | `cmd` | `pattern` | Claude render (`permissions.allow`) | Gemini render (`tools.allowed`) |
|--------|-------|-----------|-------------------------------------|---------------------------------|
| `shell` | executable | arg glob | `Bash(cmd pattern)` | `run_shell_command(cmd)` |
| `file_read` | — | path glob | `Read(pattern)` | `read_file(pattern)` |
| `file_write` | — | path glob | `Write(pattern)` | `write_file(pattern)` |
| `network` | — | url glob | `WebFetch(pattern)` | *(skipped)* |
| `mcp_tool` | server | tool | `mcp__server__tool` | *(skipped)* |
| `wildcard` | — | — | `*` | *(skipped)* |

Claude Code renders all six rule kinds; Gemini CLI renders only `shell` / `file_read` / `file_write` and skips the rest (it has no canonical network / mcp-tool / wildcard allowlist syntax). `deny` rules are not yet supported by any target tool's allowlist and are ignored on sync.

### Commands

- `agentspec permissions init [--force]`: scaffold `~/.agents/permissions.yml` with example rules.
- `agentspec permissions sync [--tool <slug>] [--dry-run]`: translate the profile into Claude's `permissions.allow` (`~/.claude/settings.json`) and Gemini's `tools.allowed` (`~/.gemini/settings.json`). `--dry-run` shows changes without writing.
- `agentspec permissions show [--tool <slug>]`: display the profile and per-tool rendered allowlists.

### Sentinel-Key Merge

Sync is non-destructive: pre-existing user entries are preserved. agentspec tracks the entries it manages via a parallel **sentinel key** in each settings file (`permissions.__agentspec_managed` for Claude, `tools.__agentspec_managed` for Gemini), so a sync only adds or removes agentspec-managed entries and never clobbers entries the user added by hand.

---

## Hooks

Portable lifecycle hook scripts.

### Location

| Path | Role |
|------|------|
| `~/.agents/hooks/` | Canonical store (source of truth) |
| `~/.claude/hooks/` | Claude Code's native hooks dir (the only tool hooks dir today) |

### Commands

- `agentspec hooks add <path>`: copy a hook script into the canonical store (`~/.agents/hooks/`).
- `agentspec hooks list`: list the canonical store **and** each tool's hooks.
- `agentspec hooks link <name> [--tool <slug>] [--all-tools]`: symlink a stored hook into a tool's hooks dir. Claude Code (`~/.claude/hooks/`) is the only tool with a hooks directory today; the design is extensible as other tools gain one.

---

## Plugins

Inventory and export of installed Claude Code plugins.

### Source

`~/.claude/plugins/installed_plugins.json` (Claude Code).

### Commands

- `agentspec plugins list`: inventory installed Claude Code plugins (each shown as `plugin@marketplace` with version, git SHA, and scope).
- `agentspec plugins export [-o <file>]`: write a portable plugin manifest. Defaults to `~/.agents/plugins.yml` unless `-o`/`--output` overrides the path.

---

## Cross-Resource Comparison

| Resource | File Pattern | Format | Discovery Strategy |
|----------|-------------|--------|-------------------|
| Skills | `{name}/SKILL.md` | YAML + Markdown | `SKILL.md` filename match |
| Agents | `agents/{name}.md` | YAML + Markdown | `agents/` parent dir + frontmatter validation (`.md` only) |
| AGENTS.md | `AGENTS.md` | Markdown | Filename match in project roots |
| CLAUDE.md | `CLAUDE.md` | Markdown | Directory tree walk |
| llms.txt | `llms.txt` | Markdown | Filename match in project roots |
| Instruction files | `.cursorrules`, `GEMINI.md`, etc. | Markdown | Known filename match per tool |
| Plans | `~/.agents/plans/{name}.md` | YAML + Markdown | Scan `~/.agents/plans/*.md` |
| Memories | `memory/*.md` | YAML + Markdown | Known Claude Code project paths |
| Sessions | tool-native (JSONL / SQLite) | tool-specific | Known tool-specific paths (claude/codex/copilot/gemini) |
| MCP | `~/.agents/mcp/{name}.json`, `.mcp.json` | JSON | Canonical store + `.mcp.json` in project roots |
| Permissions | `~/.agents/permissions.yml` | YAML | Single canonical profile |
| Hooks | `~/.agents/hooks/*` | script | Canonical store + per-tool hooks dirs |
| Plugins | `installed_plugins.json` / `~/.agents/plugins.yml` | JSON / YAML | Claude Code plugin inventory |

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
