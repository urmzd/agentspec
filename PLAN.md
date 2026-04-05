# Universal Sessions & Settings for agentspec

## Context

agentspec currently hardcodes session support for only Claude and Codex via two separate source files (`session/claude.rs`, `session/codex.rs`) with a `get_source()` function that matches on string names. The TUI and CLI also hardcode `["claude", "codex"]` loops. There is no settings management. The goals:

1. A **canonical Session IR** (like the existing Resource IR for skills/agents) that any tool can adapt to
2. **Universal session discovery** across all installed tools (`session list` with no arg shows everything)
3. **Settings management** that reads/writes each tool's native config format
4. **IR-first approach**: clean up interfaces, define the IR, then migrate existing code

---

## Phase 1: Session IR + Adapter Trait (foundation)

### New: `src/session/ir.rs`

Define the canonical session representation:

```rust
SessionIR {
    id: String,
    tool_slug: String,          // open-ended, not a closed enum
    cwd: Option<String>,
    started_at: Option<DateTime<Utc>>,
    ended_at: Option<DateTime<Utc>>,
    first_prompt: Option<String>,
    summary: Option<String>,
    project: Option<String>,
    branch: Option<String>,
    messages: Vec<MessageIR>,
    files_touched: Vec<String>,
    tools_used: Vec<String>,
    model: Option<String>,
    extensions: HashMap<String, serde_json::Value>,  // JSON not YAML
    source_path: Option<PathBuf>,                    // not serialized
}

SessionMetaIR {
    id: String,
    tool_slug: String,
    cwd: Option<String>,
    started_at: Option<DateTime<Utc>>,
    first_prompt: Option<String>,
    summary: Option<String>,
    project: Option<String>,
    source_path: Option<PathBuf>,
}

MessageIR { role: RoleIR, content: Vec<ContentBlockIR>, timestamp: Option<DateTime<Utc>> }
RoleIR { User, Assistant, System, Tool }
ContentBlockIR {
    Text { text },
    ToolUse { name, input, id },
    ToolResult { content, tool_use_id, is_error },
    Unknown { raw: serde_json::Value },
}
```

Key decisions:
- `tool_slug: String` instead of a closed enum -- any tool can provide sessions without modifying an enum
- `extensions: HashMap<String, serde_json::Value>` -- JSON not YAML since all session formats are JSON-based
- `ContentBlockIR::Unknown` preserves data from unrecognized formats
- `SessionMetaIR` is the lightweight version for listing (no messages parsed)

### New: `src/session/adapter.rs`

```rust
trait SessionAdapter: Send + Sync {
    fn tool_slug(&self) -> &str;
    fn tool_name(&self) -> &str;
    fn session_roots(&self) -> Vec<PathBuf>;
    fn is_available(&self) -> bool;       // default: any session_root exists
    fn list_sessions(&self) -> Result<Vec<SessionMetaIR>>;
    fn load_session(&self, id: &str) -> Result<SessionIR>;
    fn latest_session(&self) -> Result<SessionIR>;  // default: first from list_sessions
}
```

Mirrors the existing `Adapter` trait pattern from `src/adapters/mod.rs`.

### Modify: `src/session/mod.rs`

- Add `pub mod ir; pub mod adapter;`
- Keep old types temporarily for backward compat during migration

---

## Phase 2: Extend CodingTool trait

### Modify: `src/tools/mod.rs`

Add to `CodingTool` trait:
- `fn sessions_dir(&self) -> Option<PathBuf>`
- `fn config_path(&self) -> Option<PathBuf>`

Expand `define_tool!` macro to accept `sessions` and `config` params.

Tool directory mapping:

| Tool | sessions_dir | config_path |
|------|-------------|-------------|
| Claude | `.claude/projects` | `.claude/settings.json` |
| Codex | `.codex/sessions` | `.codex/config.toml` |
| Gemini | `.gemini/history` | `.gemini/settings.json` |
| Copilot | `.copilot` | `.copilot/config.json` |
| Cline | `.cline/data/workspaces` | `.cline/data/globalState.json` |
| Windsurf | None | None |
| OpenHands | None | None |
| Amp | None | None |
| Cursor | None | None |
| OpenCode | None | None |
| Kimi | None | None |

---

## Phase 3: Migrate existing sources to adapters

### New: `src/session/adapters/mod.rs`

Registry:
```rust
fn all_session_adapters() -> Vec<Box<dyn SessionAdapter>>
fn available_session_adapters() -> Vec<Box<dyn SessionAdapter>>  // filtered to is_available()
fn adapter_for_tool(slug: &str) -> Option<Box<dyn SessionAdapter>>
```

Slug aliases for backward compat: `"claude"` -> `"claude-code"`.

### New: `src/session/adapters/claude.rs`

Port logic from `src/session/claude.rs` into `ClaudeSessionAdapter` implementing `SessionAdapter`.

- `session_roots()` -> `[~/.claude/projects]`
- Scans `~/.claude/projects/*/*.jsonl` (2 levels, no deeper)
- JSONL types: user, assistant, result (skip file-history-snapshot)

### New: `src/session/adapters/codex.rs`

Port logic from `src/session/codex.rs` into `CodexSessionAdapter`.

- `session_roots()` -> `[~/.codex/sessions]`
- Iterative traversal with depth limit 8 (fixes crash from infinite recursion)
- JSONL types: session_meta, response_item, event_msg

### Delete: `src/session/claude.rs`, `src/session/codex.rs`

Remove old implementations and the `Source` enum, `SessionSource` trait, `get_source()` from `mod.rs`.

---

## Phase 4: Universal discovery + CLI/TUI migration

### New: `src/session/discover.rs`

```rust
fn discover_all_sessions() -> Result<Vec<SessionMetaIR>> {
    // Enumerate available_session_adapters()
    // Call list_sessions() on each
    // Merge, sort by started_at desc
}
```

### Modify: `src/session/find.rs`

Replace hardcoded `["claude", "codex"]` with `available_session_adapters()`.

### Modify: `src/session/render.rs`

Accept `&SessionIR` instead of `&Session`. Use `tool_slug` -> `find_tool()` for display name.

### Modify: `src/cli.rs`

- `SessionAction::List { source }` becomes `SessionAction::List { tool: Option<String> }`
- When `tool` is None, list all (via `discover_all_sessions()`)
- Add `Command::Settings` with subcommands

### Modify: `src/main.rs`

Update session command handlers to use `adapter_for_tool()` / `discover_all_sessions()`.

### Modify: `src/tui/app.rs`

`load_sessions()` uses `available_session_adapters()` instead of hardcoded `["claude", "codex"]` loop. `SessionEntry.source` becomes the tool slug.

---

## Phase 5: Settings management

### New: `src/settings/mod.rs`

```rust
trait ToolSettings: Send + Sync {
    fn tool_slug(&self) -> &str;
    fn settings_path(&self) -> Option<PathBuf>;
    fn read_raw(&self) -> Result<serde_json::Value>;  // normalize TOML/JSON/YAML to JSON
    fn write_raw(&self, value: &serde_json::Value) -> Result<()>;
    fn get(&self, key: &str) -> Result<Option<serde_json::Value>>;  // dotted key path
    fn set(&self, key: &str, value: serde_json::Value) -> Result<()>;
    fn keys(&self) -> Result<Vec<String>>;
}
```

### Tool-specific settings (what exists on disk today):

| Tool | Format | Key settings |
|------|--------|-------------|
| Claude | JSON (`~/.claude/settings.json`) | model, permissions.allow, effortLevel, enabledPlugins, mcpServers, statusLine |
| Codex | TOML (`~/.codex/config.toml`) | model, model_reasoning_effort, sandbox_mode |
| Copilot | JSON (`~/.copilot/config.json`) | model, theme, experimental, include_coauthor |
| Gemini | JSON (`~/.gemini/settings.json`) | security.auth, general.previewFeatures, tools.allowed |
| Cline | JSON (`~/.cline/data/globalState.json`) | mode, autoApprovalSettings, planMode*/actMode* model configs |

### New: `src/settings/claude.rs`, `codex.rs`, `copilot.rs`, `gemini.rs`, `cline.rs`

Each reads/writes the tool's native config format. Codex requires `toml` crate (add to Cargo.toml).

### CLI commands:

```
agentspec settings list              # show all tools + their config paths
agentspec settings show <tool>       # pretty-print all settings
agentspec settings get <tool> <key>  # get specific dotted key
agentspec settings set <tool> <key> <value>  # set specific key
```

### TUI:

Add a Settings tab or integrate into the existing Tools tab to show per-tool config.

---

## Phase 6: TUI crash fix

### Modify: `src/tui/mod.rs`

- Panic hook that restores terminal state before color_eyre formats its output
- Wrap App::new() + run in a block so cleanup always runs on early error returns

### Modify: `src/session/adapters/codex.rs` (ported from old codex.rs)

- Iterative traversal with depth limit (replaces recursive `collect_jsonl_files`)

### Modify: `src/ops/discover.rs`, `src/inventory.rs`

- `max_depth` on all walkdir traversals

---

## Implementation order

1. **Phase 1** - IR types + adapter trait (additive, zero breakage)
2. **Phase 2** - Extend CodingTool (additive, zero breakage)
3. **Phase 3** - New adapter implementations + delete old sources
4. **Phase 4** - Wire up discovery, update CLI/TUI
5. **Phase 5** - Settings management (entirely new feature)
6. **Phase 6** - TUI crash fix (can be done in parallel)

---

## New dependencies

- `toml` crate for reading/writing Codex config.toml

---

## Files touched

| File | Action |
|------|--------|
| `src/session/ir.rs` | CREATE |
| `src/session/adapter.rs` | CREATE |
| `src/session/adapters/mod.rs` | CREATE |
| `src/session/adapters/claude.rs` | CREATE |
| `src/session/adapters/codex.rs` | CREATE |
| `src/session/discover.rs` | CREATE |
| `src/settings/mod.rs` | CREATE |
| `src/settings/claude.rs` | CREATE |
| `src/settings/codex.rs` | CREATE |
| `src/settings/copilot.rs` | CREATE |
| `src/settings/gemini.rs` | CREATE |
| `src/settings/cline.rs` | CREATE |
| `src/session/mod.rs` | MODIFY (add modules, remove old types) |
| `src/session/find.rs` | MODIFY (use adapter registry) |
| `src/session/render.rs` | MODIFY (accept SessionIR) |
| `src/tools/mod.rs` | MODIFY (add sessions_dir, config_path) |
| `src/cli.rs` | MODIFY (optional tool arg, settings command) |
| `src/main.rs` | MODIFY (new command handlers) |
| `src/tui/app.rs` | MODIFY (dynamic adapter loading) |
| `src/tui/mod.rs` | MODIFY (panic hook) |
| `src/session/claude.rs` | DELETE |
| `src/session/codex.rs` | DELETE |
| `Cargo.toml` | MODIFY (add toml crate) |

---

## Verification

1. `cargo build` succeeds with no warnings
2. `cargo clippy` passes
3. `agentspec session list` shows sessions from all installed tools
4. `agentspec session list claude-code` filters to Claude only
5. `agentspec session list claude` works (alias)
6. `agentspec session find` fuzzy-searches across all tools
7. `agentspec session export claude-code --last` exports markdown
8. `agentspec settings show claude-code` prints Claude settings
9. `agentspec settings show codex` reads TOML config
10. `agentspec settings list` shows all tools + config paths
11. TUI Sessions tab shows all tool sessions
12. TUI doesn't crash / restores terminal on panic
13. `agentspec` piped (non-TTY) outputs JSON with all skills
