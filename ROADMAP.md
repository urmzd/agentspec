# Roadmap

Planned features and improvements for agentspec.

## Planned

- **Native session-format writers** — go beyond portable markdown handoffs and write directly into target tools' native session stores once their formats permit safe injection.
- **Permission deny rules** — extend the portable profile with deny semantics once Claude and Gemini expose a deny-list surface (today only allow rules are translated).
- **Memory adapters for non-Claude tools** — sync memories for other tools as they gain memory stores, beyond today's Claude Code support.
- **TUI surfaces for plans, MCP, and permissions** — add interactive views for the plan store, MCP registrations, and permission profiles (these are CLI-only today; the Configs TUI tab covers per-project file-readiness only).

## Shipped

Everything below landed in v0.10.0 — see [CHANGELOG.md](CHANGELOG.md) for details:

resource update (`manage update`), MCP server management (`mcp` with a canonical store at `~/.agents/mcp/`), permission profile sync (`permissions`), cross-tool session sync and import (`session sync` / `session import`), memory sync (`manage memory --pull/--push`), hook management (`hooks`), plugin tracking (`plugins`), planning-artifact import (`plans`), and enriched Copilot session exports.
