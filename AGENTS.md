# AGENTS.md

## Identity

`agentctl` is a Rust CLI + TUI for managing agentskills.io skills and sub-agent definitions across AI coding tools.

## Architecture

Single binary, async runtime (tokio). Key layers:

| Layer | Path | Role |
|-------|------|------|
| IR | `src/ir.rs` | Canonical `Resource` type -- all vendor formats convert to/from this |
| Adapters | `src/adapters/` | Vendor-specific parsers/emitters (agentskills, claude, gemini) |
| Tools | `src/tools/mod.rs` | `CodingTool` trait + macro-defined impls for 11 AI tools |
| Ops | `src/ops/` | CLI operations (install, link, list, validate, search, create, remove) |
| TUI | `src/tui/` | ratatui screens (skill list, agent list, tool list, link picker) |
| CLI | `src/cli.rs` | clap derive command definitions |
| Lock | `src/lockfile.rs` | `.skill-lock.json` v3 serde (backwards compatible) |

## Key Files

- `src/ir.rs` -- the canonical IR that all adapters target
- `src/adapters/mod.rs` -- `Adapter` trait definition
- `src/tools/mod.rs` -- `CodingTool` trait + `define_tool!` macro
- `src/ops/link.rs` -- symlink creation with relative path computation
- `src/tui/app.rs` -- TUI state machine

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
