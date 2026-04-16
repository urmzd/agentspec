# AGENTS.md

## Identity

`agentspec` is a Rust CLI + TUI for managing agentskills.io skills and sub-agent definitions across AI coding tools.

## Architecture

Single binary, async runtime (tokio). Key layers:

| Layer | Path | Role |
|-------|------|------|
| IR | `src/ir.rs` | Canonical `Resource` type -- all vendor formats convert to/from this |
| Adapters | `src/adapters/` | Vendor-specific parsers/emitters (agentskills, claude, gemini) |
| Tools | `src/tools/mod.rs` | `CodingTool` trait + macro-defined impls for 11 AI tools |
| Ops | `src/ops/` | CLI operations (manage, link, sync, discover, verify, prune, project_sync) |
| TUI | `src/tui/` | ratatui screens (skill list, agent list, tool list, link picker) |
| CLI | `src/cli.rs` | clap derive command definitions |
| Lock | `src/lockfile.rs` | `.skill-lock.json` v3 serde (backwards compatible) |

## Key Files

- `src/ir.rs` -- the canonical IR that all adapters target
- `src/adapters/mod.rs` -- `Adapter` trait definition
- `src/tools/mod.rs` -- `CodingTool` trait + `define_tool!` macro
- `src/ops/link.rs` -- symlink creation, relative path computation, link reconciliation
- `src/ops/sync.rs` -- full pipeline (discover, adopt, reconcile, link, verify)
- `src/ops/manage.rs` -- `manage add` source resolution (local path, git URL, owner/repo)
- `src/inventory.rs` -- `Config` struct, `TrackedResource`, SHA-256 hashing
- `src/tui/app.rs` -- TUI state machine

## Linking Mechanism

Resources live in the shared store (`~/.agents/skills/`, `~/.agents/agents/`). Tools get relative symlinks pointing into the shared store.

**`manage add <source> --all-tools`** resolves the source (local path, `owner/repo`, or git URL with optional `#branch@subpath`), copies to shared store, then creates symlinks in every installed tool's directory. Existing names are skipped (first-installed wins).

**`sync`** runs a 3-phase pipeline:
1. **Reconcile** -- adopts existing symlinks on disk that aren't tracked in config (handles links created by older versions or external tools)
2. **Link** -- creates missing symlinks for managed resources not yet present in tool directories
3. **Verify** -- checks SHA-256 hashes against stored values to detect external modifications

**Relative symlinks** are computed via `pathdiff::diff_paths()` so they survive home directory moves. Example: `~/.claude/skills/my-skill` -> `../../.agents/skills/my-skill`.

**Copy fallback** via `--copy` flag for filesystems that don't support symlinks.

## Commands

```sh
just check     # fmt + clippy + test
just ci        # fmt + clippy + build + test
just run       # cargo run
cargo build    # debug build
```

## Code Style

- Rust 2024 edition
- `cargo fmt` + `cargo clippy -- -D warnings`
- Conventional commits (enforced by pre-commit hook)
- Minimize allocations; prefer `&str` over `String` in function signatures

## Adding a New Vendor Adapter

1. Create `src/adapters/<vendor>.rs`
2. Implement the `Adapter` trait (`parse`, `emit`, `validate`)
3. Register in `src/adapters/mod.rs` (`all_adapters()` and `adapter_for_path()`)
4. Add corresponding `CodingTool` impl in `src/tools/mod.rs` via `define_tool!` macro

## Adding a New AI Tool

1. Add one line in `src/tools/mod.rs` using the `define_tool!` macro
2. Add it to `all_tools()` vec
