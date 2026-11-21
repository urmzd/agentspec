---
name: agentspec-usage
description: How and when to use the agentspec CLI to manage skills, agents, MCP servers, sessions, memories, and project config across AI coding tools. Use when asked to install or share a skill, register an MCP server everywhere, find a past AI session, sync agent resources between Claude Code, Codex, Copilot, Gemini, Cursor, and friends, group skills into profiles and turn them on or off, find out why a skill is missing from a tool, or resolve an integrity warning.
---

# agentspec

`agentspec` keeps one canonical copy of your agent resources in `~/.agents/` and
renders them into explicitly selected AI coding tools. Register a resource once,
then choose its destinations with `manage link` or `manage add --tools`.

Run `agentspec --help` for the command map, or `agentspec commands --format json`
for the full tree as machine-readable JSON.

## Output format

Human-readable by default. Every command takes the **global** `--format json`
flag. There is no per-command `--json` flag.

```sh
agentspec status --format json
agentspec mcp list --format json
```

Progress chatter goes to stderr; `--format json` writes exactly one JSON
document to stdout, so it is always safe to pipe into `jq`.

## The canonical store

| Path | Holds |
| --- | --- |
| `~/.agents/skills/<name>/` | skills |
| `~/.agents/agents/<name>.md` | sub-agents |
| `~/.agents/mcp/<name>.json` | MCP server definitions |
| `~/.agents/hooks/` | lifecycle hook scripts |
| `~/.agents/memories/`, `projects/`, `plans/`, `sessions/` | synced-in copies |
| `~/.agents/permissions.yml` | portable permission profile |
| `~/.agents/profiles.yml` | profiles: named groups of skills and agents |

Tools get real copies of the store version by default; `--symlink` opts into
relative symlinks for skills. Agents are always rendered copies. Adoption never modifies or deletes the original.

## Start here

```sh
agentspec sync --adopt      # discover → adopt → repair selected links → verify
agentspec status            # what is managed, what is not
agentspec                   # interactive TUI
```

`sync` is the default entry point. It adopts untracked resources, re-copies
`local`-sourced ones from their origin, repairs tracked links without enabling resources in additional tools,
adopts MCP servers found in tool configs (a project `.mcp.json` stays project scoped),
refreshes tool copies that are behind the store, and reports integrity issues.

An integrity issue always needs a decision. Ordinary change does not raise one: `sync`
follows edits to project files and memories and updates made to the store by a dotfile
manager. What is left is one of three things:

| Issue | Fix |
|---|---|
| `is missing from the store` | `agentspec prune -y` drops the record and its dangling links |
| `has been modified outside agentspec` (a git or local resource) | `manage update <name>` to restore it, or `manage verify --accept --name <name>` to keep it |
| `(copy in <tool>) differs from the store` (edited in the tool directory) | move the edit into the store, or `manage verify --repair --name <name>` to rewrite the copy (the old copy is backed up) |

Edit a skill in the store or its origin, never in a tool directory.

## Skills and agents

```sh
agentspec manage add owner/repo --all-tools        # from GitHub
agentspec manage add ./my-skill --all-tools        # from a local path
agentspec manage add owner/repo#branch@subdir      # branch and subpath
agentspec manage transfer <name|path> --to <project>  # copy into a project (store or another project)
agentspec manage transfer <name> --to <project> --move  # demote from global to one project
agentspec manage transfer <name> --to . --from claude-code --tools codex  # tool to tool, same project
agentspec manage list                              # what is managed
agentspec manage link <name> <tool-slug>           # link one resource to one tool
agentspec manage verify                            # what needs a person; --repair rewrites edited copies
agentspec manage create <name> --kind skill        # scaffold from template
agentspec prune -y                                 # drop broken links
```

Kind is auto-detected; pass `--kind` only to override.

## Profiles: unload skills you are not using

A tool lists every linked skill at the start of every session, so each one costs
context whether or not it is used. A profile groups skills and agents by the
kind of work they serve. Turning it off removes their links from every tool. The
store keeps them, and turning it on restores the same links.

```sh
agentspec profile list                              # state and member counts
agentspec profile show <name>                       # members and where each is linked
agentspec profile create web style-brand ai-elements --description "Websites and UI"
agentspec profile off web                           # unload from every tool
agentspec profile on web                            # load again, same links as before
agentspec profile add web <name>                    # skill:<name> or agent:<name> when a name is both
```

- A resource in no profile is always linked. One in several profiles stays linked
  while any of them is on.
- **A skill you need is not available?** Run `agentspec profile list`. If its profile
  is off, `agentspec profile on <name>` brings it back. Turn it off again when the
  work is done if it was off before.
- Do not turn a profile off to work around a problem with one skill. Ask first: the
  user chose what loads.
- `enabled: false` in `~/.agents/profiles.yml` is the default for every machine;
  `profile on` and `profile off` record this machine's own choice.

## MCP servers

One definition, every tool, whatever dialect that tool speaks — `mcpServers`
JSON, Amp's `amp.mcpServers` key, Codex's `[mcp_servers.<name>]` TOML tables, or
OpenCode's `mcp` key. agentspec translates on write.

```sh
agentspec mcp add sr --command sr --args "mcp serve"      # stdio server
agentspec mcp add api --url https://mcp.example/sse --type http
agentspec mcp add sr --command sr --tool github-copilot   # one tool only
agentspec mcp list                                         # store + per-tool state
agentspec mcp doctor                                       # every target, path, dialect
agentspec mcp adopt                                        # pull tool-native servers into the store
agentspec mcp adopt --project <dir> --name <server>        # promote a project server to every tool
agentspec mcp sync                                         # push the store to every tool
agentspec mcp unlink sr --tool cursor                      # remove from one tool
agentspec mcp remove sr                                    # remove everywhere
```

`mcp doctor` is the first thing to run when a server is not showing up: it lists
every MCP-capable tool, whether it is installed, its config path, and its dialect.

## Finding past sessions

`session search` scans transcripts from Claude Code, Codex, GitHub Copilot, and
Gemini CLI. Filter before you read.

```sh
# What did I ask about rate limiting, in my own words only?
agentspec session search "rate limit" --role user

# Last week's Copilot work in one repo, as JSON
agentspec session search --source copilot --project agentspec --since 7d --format json

# Sessions that ran a specific tool, or touched a specific file
agentspec session search --tool-used Bash --limit 50
agentspec session search "panic" --file src/mcp/mod.rs

# Regex, more hits per session, wider excerpts
agentspec session search "cargo\s+build" --regex --hits 10 --context 400
```

| Flag | Effect |
| --- | --- |
| `--role user\|assistant\|system\|tool` | only match messages from these roles (`human` aliases `user`) |
| `--source claude\|codex\|copilot\|gemini` | restrict to one tool, repeatable |
| `--project <substr>` | match project name or cwd |
| `--file <substr>` | session touched a matching path |
| `--tool-used <name>` | session called that tool |
| `--since` / `--until` | `2026-01-31`, RFC 3339, or a relative age like `7d`, `24h` |
| `--limit` / `--hits` / `--context` | sessions returned, matches per session, excerpt width |
| `--regex`, `--case-sensitive`, `--full` | pattern mode, exact case, whole matched messages |

Every JSON result carries an `export_command`, so the follow-up is mechanical:

```sh
agentspec session search "flaky test" --role user --format json \
  | jq -r '.results[0].export_command' | sh
```

Other session verbs:

```sh
agentspec session list                    # newest sessions across all sources
agentspec session list --source claude --limit 50
agentspec session find                    # interactive fuzzy picker
agentspec session export claude --last -o handoff.md
agentspec session sync claude codex --last # portable handoff staged for another tool
```

## Everything else

```sh
agentspec project sync                 # AGENTS.md / CLAUDE.md / llms.txt into the store
agentspec manage memory --pull         # pull tool memories into ~/.agents/memories
agentspec permissions init && agentspec permissions sync
agentspec hooks add ./on-stop.sh && agentspec hooks link on-stop.sh --all-tools
agentspec plugins export
agentspec worktree create <name>       # <repo>/.worktrees/<name>
agentspec fleet doctor                 # multi-agent fleet backend check
```

## Gotchas

- `--format json` is global and must be accepted anywhere, but only commands that
  produce data emit JSON; action commands report progress on stderr.
- `manage add` skips a name that already exists — first installed wins. Remove it
  first to replace it.
- A tool counts as installed when any of its config directories exists. Run
  `agentspec mcp doctor` to see what agentspec thinks is present.
- Adoption is non-destructive: originals are never edited or deleted, and a
  definition already in the store wins over a tool-native or project one.
- The inventory is `config.yml` in the OS config directory:
  `~/Library/Application Support/agentspec/` on macOS, `~/.config/agentspec/` on Linux.
  Read state with `agentspec status --format json` instead of parsing that file.
- When a dotfile manager deploys `~/.agents/`, register resources from the deployed
  tree (`manage add ~/.agents/skills/<name>`), never from the manager's source
  directory, and edit the skill in that source. `sync` then follows the change.
- Before scripting against `--format json`, print one document and read its keys.
  Do not guess field names.
