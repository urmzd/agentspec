---
name: resource-conventions
description: Reference for the file formats, locations, and discovery conventions agentspec manages across AI coding tools (skills, agents, AGENTS.md, CLAUDE.md, llms.txt, instruction files, plans, memories, sessions, MCP, permissions, hooks, plugins). Use when you need to know where a resource lives, what format it uses, how agentspec discovers it, or how it maps into the ~/.agents canonical store.
---

# Resource Conventions

Portable reference for every resource type agentspec manages. agentspec normalizes
resources into an intermediate representation (IR) with **8 ResourceKinds** (`Skill`,
`Agent`, `ProjectConfig`, `InstructionFile`, `LlmsTxt`, `Memory`, `Session`, `Plan`)
and supports **11 tools**: Claude Code, Cline, Windsurf, OpenHands, Gemini CLI,
GitHub Copilot, Amp, Cursor, Codex, OpenCode, Kimi CLI. All 11 support skills + agents
dirs; **9** also host MCP servers (see the MCP section). Run `agentspec tools` for the
live table of what is installed on the current machine.

## Canonical store (`~/.agents/`)

The shared cross-tool store. agentspec is source of truth for managed kinds (skills,
agents) and keeps copies of synced-in kinds.

- `~/.agents/skills/{name}/SKILL.md`: managed skills
- `~/.agents/agents/{name}.md`: managed agents
- `~/.agents/projects/<key>/`: project config + instruction-file snapshots (key = full
  project path encoded Claude-style, so same-basename projects never collide)
- `~/.agents/memories/<project>/`: pulled tool memories
- `~/.agents/plans/<name>.md`: planning artifacts
- `~/.agents/sessions/<target>/<id>.md`: portable handoffs
- `~/.agents/mcp/<name>.json`: stored MCP server definitions
- `~/.agents/hooks/`: copied hook scripts
- `~/.agents/permissions.yml`: portable permission profile (created on demand)
- `~/.agents/plugins.yml`: exported plugin manifest (created on demand)

Config dir: `~/.config/agentspec/config.yml`.

## Skills (agentskills.io)

- **Location**: `{name}/SKILL.md`. Tool: `.{tool}/skills/{name}/`; cross-tool:
  `.agents/skills/{name}/` or `~/.agents/skills/{name}/`.
- **Format**: YAML frontmatter (`name` must match parent dir; `description`) + Markdown.
  Optional `scripts/`, `references/`, `assets/`.
- **Discovery**: match `SKILL.md` filename during filesystem walk; parent dir = name.

## Agents / Subagents

- **Location**: `agents/{name}.md` (Claude Code, Gemini CLI). Tool: `.claude/agents/`,
  `.gemini/agents/`; shared: `~/.agents/agents/{name}.md`.
- **Format**: YAML frontmatter + Markdown (`name` + `description` required).
- **Discovery**: scan `.md` files inside `agents/` dirs; validate required frontmatter.
- **Codex TOML**: Codex defines agents as `agents/{name}.toml`, but agentspec does not
  yet parse or emit them; discovery is `.md`-only. The TOML schema in
  `docs/resources.md` is a spec reference for a future addition.

## AGENTS.md (Project Config)

- **Location**: project root `./AGENTS.md` (nested files resolve closest-wins in monorepos).
- **Format**: plain Markdown, no required frontmatter; a tool-agnostic README for agents.
- **Discovery**: filename match in project roots (dirs containing `.git`).

## CLAUDE.md (Claude Code Config)

- **Location**: `./CLAUDE.md` or `./.claude/CLAUDE.md`, `~/.claude/CLAUDE.md`,
  `./CLAUDE.local.md` (gitignored), plus managed-policy paths.
- **Format**: plain Markdown; supports `@path` imports (max 5 hops) and
  `.claude/rules/*.md` path-scoped rules.
- **Discovery**: walk up the directory tree from cwd checking each level.

## llms.txt

- **Location**: project root `./llms.txt` (and optional `/llms-full.txt`).
- **Format**: Markdown in required order: H1 name, optional blockquote summary,
  optional body, optional H2 link sections (`[name](url): description`), optional `Optional` H2.
- **Discovery**: filename match in project roots.

## Instruction Files

Per-tool config files each tool reads natively (mirrors `PROJECT_FILES` in
`crates/agentspec/src/project_files.rs`).

- **Files**: `CLAUDE.md` (Claude Code), `GEMINI.md` (Gemini CLI),
  `.github/copilot-instructions.md` (GitHub Copilot), `codex.md` (Codex; global
  `~/.codex/instructions.md`), `.cursorrules` and `.cursor/rules/` directory (Cursor),
  `.clinerules` (Cline), `.windsurfrules` (Windsurf).
- **Location**: project root (or `.github/` for Copilot, `.cursor/` for Cursor rules).
- **Format**: plain Markdown, no required frontmatter.
- **Discovery**: filename match against the known list; owning tool derived from filename.
  `project sync` snapshots these into `~/.agents/projects/<project>/`.

## Plans

- **Location**: `~/.agents/plans/<name>.md`.
- **Format**: Markdown, optional YAML frontmatter. Gemini antigravity imports use
  frontmatter (`name`, `description`, `source: gemini-antigravity`, `session`,
  `artifact_type`, `updated_at`) via `plans import [gemini]`; `plans list` shows the store.
- **Discovery**: scan `~/.agents/plans/*.md`; `status`/`manage list` surface untracked
  plans as unmanaged. Plans are NOT shown in the Configs TUI tab (that tab shows per-project
  file-readiness indicators only).

## Memories (Claude Code)

- **Location**: `~/.claude/projects/<project>/memory/` (project path derived from git root;
  worktrees share one dir). `MEMORY.md` is the index; `{topic}.md` loaded on demand.
- **Format**: Markdown with YAML frontmatter (`name`, `description`, `type`: user/feedback/
  project/reference).
- **Discovery**: scan `~/.claude/projects/*/memory/*.md`. `manage memory --pull` copies into
  `~/.agents/memories/<project>/`; `--push` copies back into matching project dirs.

## Sessions

- **Sources (4)**: claude, codex, copilot, gemini. Native stores are append-only/tool-internal.
- **Format**: tool-specific JSON/JSONL sources rendered to portable Markdown handoffs.
  Copilot is enriched from `~/.copilot/session-store.db` (SQLite) with summary, repository,
  branch, Files Touched, Checkpoints, and References (missing DB = graceful no-op).
- **Staging**: `session sync <source> <target> [<id>] [--last]` and
  `session import <target> <file>` stage handoffs at `~/.agents/sessions/<target>/<id>.md`.
- **Query**: `session list [source]` lists newest-first metadata across one source or all
  of them (`--project`, `--since`, `--until`, `--limit`). `session search [query]` scans
  transcripts: metadata filters (`--source`, `--project`, `--since`, `--until`) run before
  any transcript is opened, then `--role`, `--file`, `--tool-used`, and the query text scan
  the survivors message by message. `--role user` restricts hits to what the human typed.
  Every JSON result carries an `export_command` for the follow-up.

## MCP (Model Context Protocol)

- **Canonical store**: `~/.agents/mcp/<name>.json`, always in the `mcpServers` shape.
  Projects may also declare servers in a root `.mcp.json`.
- **Server types**: `stdio` (`command`/`args`/`env`), `http`/`sse` (`url`). stdio XOR remote.
- **Dialects**: agentspec stores one definition and translates it into whatever the
  target tool speaks.

  | Tool | Config path | Dialect | Key |
  | --- | --- | --- | --- |
  | Claude Code | `.claude/settings.json` | json | `mcpServers` |
  | Gemini CLI | `.gemini/settings.json` | json | `mcpServers` |
  | Cursor | `.cursor/mcp.json` | json | `mcpServers` |
  | GitHub Copilot | `.copilot/mcp-config.json` | json | `mcpServers` |
  | Windsurf | `.codeium/windsurf/mcp_config.json` | json | `mcpServers` |
  | Cline | `.cline/mcp_settings.json` | json | `mcpServers` |
  | Amp | `.config/amp/settings.json` | json | `amp.mcpServers` (one literal key) |
  | Codex | `.codex/config.toml` | toml | `[mcp_servers.<name>]` |
  | OpenCode | `.config/opencode/opencode.json` | opencode-json | `mcp`, argv array |

  Codex writes go through `toml_edit`, so surrounding comments and formatting survive.
  OpenHands and Kimi CLI have no MCP target.
- **Commands**: `mcp add <name> [--command|--url] [--args] [--env] [--type] [--tool]`,
  `mcp remove <name>` (everywhere), `mcp list`, `mcp link <name> [--tool|--all-tools]`,
  `mcp unlink <name> [--tool]` (tool configs only, store kept), `mcp sync`,
  `mcp adopt` (pull tool-native servers into the store), `mcp doctor` (every target,
  path, dialect, and install state).
- **Discovery**: `agentspec sync` adopts servers from both project `.mcp.json` files and
  each tool's own config into the canonical store (originals untouched, store wins), then
  links every canonical server into all MCP-capable installed tools.

## Permissions

- **Location**: portable profile at `~/.agents/permissions.yml`.
- **Format**: rule kinds `shell`, `file_read`, `file_write`, `network`, `mcp_tool`, `wildcard`.
- **Sync**: `permissions sync [--tool] [--dry-run]` translates the profile into Claude's
  `permissions.allow` (`~/.claude/settings.json`) and Gemini's `tools.allowed`
  (`~/.gemini/settings.json`). Claude renders all kinds (`Bash(...)`, `Read(...)`, `Write(...)`,
  `WebFetch(...)`, `mcp__srv__tool`, `*`); Gemini renders shell/file_read/file_write only.
  Pre-existing user entries are preserved via a sentinel key. `permissions init [--force]`
  scaffolds; `permissions show [--tool]` displays.

## Hooks

- **Location**: canonical scripts in `~/.agents/hooks/`. Today the only tool hooks dir is
  Claude Code's `~/.claude/hooks/`.
- **Commands**: `hooks add <path>` copies a script in; `hooks list` shows the store + per-tool;
  `hooks link <name> [--tool|--all-tools]` symlinks a stored hook into a tool's hooks dir.

## Plugins (Claude Code)

- **Source**: `~/.claude/plugins/installed_plugins.json` (plugin@marketplace, version, git SHA,
  scope).
- **Commands**: `plugins list` inventories installed plugins; `plugins export [-o <file>]`
  writes a portable manifest (default `~/.agents/plugins.yml`).
