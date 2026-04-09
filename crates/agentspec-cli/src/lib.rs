use agentspec_update::UpdateResult;
use clap::Command;
use clap_complete::{Shell, generate};
use std::io;

/// Generate shell completions for a clap command and write them to stdout.
pub fn print_completions(shell: Shell, cmd: &mut Command) {
    generate(shell, cmd, cmd.get_name().to_string(), &mut io::stdout());
}

/// Run the self-update flow for a CLI tool.
///
/// `repo` — GitHub "owner/name" (e.g., "urmzd/sr").
/// `current_version` — semver string without "v" prefix.
/// `binary_name` — asset name prefix (e.g., "sr").
pub fn run_update(repo: &str, current_version: &str, binary_name: &str) -> anyhow::Result<()> {
    eprintln!("current version: {current_version}");

    match agentspec_update::self_update(repo, current_version, binary_name)? {
        UpdateResult::AlreadyUpToDate => {
            eprintln!("already up to date ({current_version})");
        }
        UpdateResult::Updated { from, to } => {
            eprintln!("updated: {from} -> {to}");
        }
    }

    Ok(())
}

/// Check if stdout is connected to a terminal.
pub fn is_tty() -> bool {
    io::IsTerminal::is_terminal(&io::stdout())
}

/// Output format for CLI tools that support both human and machine output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum OutputFormat {
    /// Human-readable output (default when TTY).
    Human,
    /// JSON output for piping to other tools.
    Json,
}

impl OutputFormat {
    /// Auto-detect output format based on TTY status.
    pub fn auto() -> Self {
        if is_tty() {
            Self::Human
        } else {
            Self::Json
        }
    }
}
