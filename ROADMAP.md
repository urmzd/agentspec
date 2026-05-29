# Roadmap

Planned features and improvements for agentspec.

## Done

### Resource update

`manage update [name]` re-pulls git-sourced resources (re-clone) and re-copies local-sourced ones, refreshing the SHA-256 hash and `updated_at` and re-propagating copy-strategy links. Discovered resources are skipped (no upstream). Omit `name` to update everything.

### MCP server management

A canonical MCP store lives at `~/.agents/mcp/<name>.json`. `mcp add <name>` registers a server (`--command`/`--args`/`--env` for stdio, `--url`/`--type` for http/sse) into the store and injects it into every MCP-capable installed tool (claude-code, gemini-cli, cursor) or just `--tool`. `mcp remove` strips it from tool configs and deletes the store file (with `--purge` or no `--tool`); `mcp list` shows the store plus each tool's registrations; `mcp link` injects a stored server into tool config(s); `mcp sync` links every canonical server into all MCP-capable tools. `agentspec sync` additionally auto-discovers project `.mcp.json` files.

### Permission profile sync

`permissions init` scaffolds a portable `~/.agents/permissions.yml` (rule kinds: shell, file_read, file_write, network, mcp_tool, wildcard). `permissions sync [--tool] [--dry-run]` translates the profile into Claude's `permissions.allow` and Gemini's `tools.allowed` — Claude renders all rule kinds, Gemini renders shell/file_read/file_write and skips the rest. Pre-existing user entries are preserved via a sentinel so sync only adds/removes agentspec-managed entries. `permissions show` prints the profile and the per-tool rendered allowlists.

### Cross-tool session sync and import

`session sync <source> <target> [<id>] [--last]` loads a session from a source tool, reuses the read adapter + IR, renders a portable markdown handoff, and stages it at `~/.agents/sessions/<target>/<id>.md`. `session import <target> <file>` stages an external markdown handoff for a target tool. Because native session stores are append-only and tool-internal, agentspec stages a portable handoff keyed by the target rather than fabricating a native session file.

### Memory sync

`manage memory [--project] [--type]` lists Claude Code memories by default; `--pull` copies tool memories into `~/.agents/memories/<project>/` and `--push` copies canonical memories back into matching Claude project memory dirs.

### Hook management

`hooks add <path>` copies a hook script into `~/.agents/hooks/`, `hooks list` shows the canonical store plus per-tool hooks, and `hooks link <name> [--tool] [--all-tools]` symlinks a stored hook into a tool's hooks dir (today Claude Code's `~/.claude/hooks/` is the only tool hook dir).

### Plugin tracking

`plugins list` inventories Claude Code plugins from `~/.claude/plugins/installed_plugins.json` (plugin@marketplace, version, git SHA, scope). `plugins export [-o <file>]` writes a portable manifest (default `~/.agents/plugins.yml`) for reproducible machine setup.

### Planning artifact sync

`plans import [gemini]` imports Gemini CLI antigravity planning artifacts (task, implementation plan, walkthrough, via the `.resolved` view + `.metadata.json`) into `~/.agents/plans/<artifact>-<short-uuid>.md` with YAML frontmatter. Plans are also discovered: `status` and `manage list` scan `~/.agents/plans/*.md` and surface untracked plans as unmanaged. `plans list` lists the canonical plan store.

### Richer Copilot session data

Copilot session exports are enriched from `~/.copilot/session-store.db` (SQLite). Exports now include the session summary, repository (as project), branch, files touched, checkpoints (overview/work-done/technical-details/next-steps), and references — pulled from the sessions/session_files/checkpoints/session_refs tables. `events.jsonl` remains the message source, and a missing DB is a graceful no-op.

## Planned

- **Native session-format writers** — go beyond portable markdown handoffs and write directly into target tools' native session stores once their formats permit safe injection.
- **Permission deny rules** — extend the portable profile with deny semantics once Claude and Gemini expose a deny-list surface (today only allow rules are translated).
- **Memory adapters for non-Claude tools** — sync memories for other tools as they gain memory stores, beyond today's Claude Code support.
- **TUI surfaces for plans, MCP, and permissions** — add interactive views for the plan store, MCP registrations, and permission profiles (these are CLI-only today; the Configs TUI tab covers per-project file-readiness only).
