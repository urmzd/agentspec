# Roadmap

Planned features and improvements for agentspec.

## Planned

### MCP server management

Discover, adopt, and sync MCP server definitions across tools. Today MCP configs are siloed:

- Claude Code: `mcpServers` key in `~/.claude/settings.json`
- Gemini CLI: `~/.gemini/antigravity/mcp_config.json`
- Per-project: `.mcp.json` files in repo roots
- Plugin marketplace: `.mcp.json` bundled with plugins (context7, terraform, discord, gitlab)

agentspec would:

- **`manage mcp list`** — discover MCP servers across all tool configs and `.mcp.json` files.
- **`manage mcp add <name>`** — add an MCP server to the shared store (`~/.agents/mcp/`).
- **`manage mcp link <name> <tool>`** — inject the server definition into a tool's settings.
- **Canonical format** — store MCP servers as individual JSON files with `command`, `args`, `env`, `url` fields. Adapters translate to each tool's settings format on link.

### Permission profile sync

Claude Code and Gemini CLI both maintain tool allowlists with nearly identical patterns expressed in different syntax:

- Claude: `permissions.allow` in settings.json — `Bash(cat *)`, `Bash(ls *)`, etc.
- Gemini: `tools.allowed` in settings.json — `run_shell_command(cat)`, `run_shell_command(ls)`, etc.

agentspec would:

- **`manage permissions`** — define a portable permission profile in `~/.agents/permissions.yml`.
- **`sync`** — translate the canonical profile into each tool's native format and inject into settings.
- Ensure consistent security posture across all tools without manual duplication.

### Cross-tool session sync

Export and import full session context between AI coding tools. Today `session export` dumps a session to markdown, but there is no way to **import** that context into another tool's session format. This feature would enable:

- **`session sync <source> <target>`** — translate a session from one tool's format to another (e.g., Claude → Codex, Codex → Gemini).
- **`session import <source> <file>`** — import a markdown handoff into a tool's native session store.
- **Adapter-based translation** — reuse the existing IR and adapter layer to convert between vendor session formats, just as we do for skills and agents today.
- **Bidirectional sync** — keep sessions mirrored across tools so switching mid-task is seamless.

This would close the loop on the session workflow: discover → export → **import → resume** in any supported tool.

### Memory sync across tools

Extend the memory browser (`manage memory`) to sync memories between tools that support them, not just browse Claude Code's store.

### Hook management

Claude Code supports custom hooks (`~/.claude/hooks/`). As other tools add hook support, agentspec would:

- Store hooks in `~/.agents/hooks/` as the canonical location.
- Link/adapt hooks to each tool's native format.
- Enable portable automation (e.g., post-session scripts, lint hooks) across tools.

### Planning artifact sync

Gemini CLI stores rich planning artifacts in `~/.gemini/antigravity/brain/` — task definitions, implementation plans, and walkthroughs with version history. agentspec would:

- Discover and export these as portable markdown documents.
- Enable import into other tools' context systems (e.g., Claude Code plans, Codex sessions).
- Preserve metadata (timestamps, revision history) across syncs.

### Plugin tracking

Claude Code has a plugin ecosystem (`~/.claude/plugins/installed_plugins.json`) with versioned installs and git SHAs. agentspec would:

- **`manage plugins list`** — inventory installed plugins across tools.
- **`manage plugins export`** — export a reproducible plugin manifest for machine setup.
- Track plugin versions for reproducibility across machines.

### Richer Copilot session data

The Copilot session adapter currently reads `events.jsonl` but the SQLite database (`session-store.db`) contains richer data:

- **Checkpoints** — title, overview, work done, technical details, next steps.
- **Session files** — which files each session touched, by which tool.
- **Session refs** — cross-references (issues, PRs, commits) from sessions.

Incorporating this data would produce more complete session exports and handoffs.

### Resource update

`manage update` — re-pull git-sourced resources and refresh hashes without removing and re-adding.
