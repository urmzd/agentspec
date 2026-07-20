# AGENTS.md

## Identity

`agentspec` is a Rust CLI + TUI for managing portable agent resources across AI coding tools: skills, agents, MCP servers, sessions, project configs/instruction files, memories, `llms.txt`, plans, permission profiles, hooks, and plugin manifests.

## Architecture

Cargo workspace with two published crates: `crates/agentspec` (the CLI + TUI binary, fully synchronous) and `crates/agentspec-sdk` (a standalone SDK for bootstrapping AI-powered CLI tools: config and CLI scaffolding helpers; not consumed by the binary).

Anchors, all under `crates/agentspec/src/`: the canonical IR is the `Resource` type and 8 `ResourceKind`s in `ir.rs`; every vendor format converts to/from it through the `Adapter` trait defined in `adapters/mod.rs`. AI tools are described by the `CodingTool` trait and the `define_tool!` macro in `tools/mod.rs`. Live inventory state (`Config`, `TrackedResource`, SHA-256 hashing as the lock) lives in `inventory.rs`; `lockfile.rs` is legacy `.skill-lock.json` serde kept for migration only. Each CLI command maps to one module under `ops/` (clap definitions in `cli.rs`), session read adapters and the session IR live under `session/`, ratatui screens under `tui/screens/`, and sentinel-keyed JSON settings edits (MCP, permissions) in `jsonfile.rs`. Use `rg` on a trait or type name to find the matching implementations.

## Linking Mechanism

Resources live in the shared store (`~/.agents/skills/`, `~/.agents/agents/`). Tools get real copies of the store version by default; `--symlink` opts into relative symlinks instead. Adoption never modifies or deletes originals: a resource adopted from a non-store location records that location as its `local` source, and `sync` re-copies source -> store -> tool links.

**`manage add <source> --all-tools`** resolves the source (local path, `owner/repo`, or git URL with optional `#branch@subpath`), copies to shared store, then copies into every installed tool's directory. Existing names are skipped (first-installed wins).

**`sync`** pipeline:
1. **Reconcile** -- adopts existing links on disk that aren't tracked in config (handles links created by older versions or external tools)
2. **Refresh** -- re-copies `local`-sourced resources from their origins into the store
3. **Link** -- creates missing tool copies for managed resources not yet present in tool directories
4. **Verify** -- checks SHA-256 hashes against stored values to detect external modifications

**Relative symlinks** (the `--symlink` strategy) are computed via `pathdiff::diff_paths()` so they survive home directory moves. Example: `~/.claude/skills/my-skill` -> `../../.agents/skills/my-skill`.

## Commands

```sh
just check     # fmt + clippy + test
just ci        # fmt + clippy + build + test
just run       # cargo run
cargo build    # debug build
```

Top-level CLI commands: `manage`, `status`, `sync`, `session`, `project`, `prune`, `mcp`, `plans`, `permissions`, `plugins`, `hooks`, `update`, `version`. The only output-format flag is the global `--format human|json` (default `human`).

## Code Style

- Rust 2024 edition
- `cargo fmt` + `cargo clippy -- -D warnings`
- Conventional commits (enforced by pre-commit hook)
- Minimize allocations; prefer `&str` over `String` in function signatures

## Adding a New Vendor Adapter

1. Create `crates/agentspec/src/adapters/<vendor>.rs`
2. Implement the `Adapter` trait (`parse`, `emit`, `validate`)
3. Register in `crates/agentspec/src/adapters/mod.rs` (add a `match` arm in `adapter_for_path()`)
4. Add corresponding `CodingTool` impl in `crates/agentspec/src/tools/mod.rs` via `define_tool!` macro
5. Add an anonymized fixture under `crates/agentspec/tests/fixtures/` and extend the conformance suite (`src/adapters/conformance_tests.rs`, or `src/session/adapters/conformance_tests.rs` for session parsers): a golden snapshot in `tests/snapshots/` plus the `parse(emit(parse(f))) == parse(f)` round-trip case. Snapshot diffs are reviewable contract changes; regenerate deliberately with `INSTA_UPDATE=always cargo test`.

## Adding a New AI Tool

1. Add one line in `crates/agentspec/src/tools/mod.rs` using the `define_tool!` macro
2. Add it to `all_tools()` vec
