use anyhow::Result;
use crossterm::style::Stylize;
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use std::io::{self, IsTerminal, Write};
use std::time::Duration;

/// Create a styled spinner for long-running operations.
pub fn spinner(message: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_draw_target(ProgressDrawTarget::stdout());
    pb.set_style(
        ProgressStyle::default_spinner()
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
            .template("  {spinner:.cyan} {msg}")
            .unwrap(),
    );
    pb.set_message(message.to_string());
    pb.enable_steady_tick(Duration::from_millis(80));
    pb
}

/// Finish a spinner, replacing it with a green checkmark line.
pub fn spinner_done(pb: &ProgressBar, detail: Option<&str>) {
    let msg = pb.message();
    pb.finish_and_clear();
    phase_ok(&msg, detail);
}

/// Print a command header with a separator line.
pub fn header(cmd: &str) {
    println!();
    println!("  {}", cmd.cyan().bold());
    println!("  {}", "─".repeat(40).dim());
    println!();
}

/// Print a completed phase with green checkmark.
pub fn phase_ok(msg: &str, detail: Option<&str>) {
    let suffix = detail
        .map(|d| format!(" · {}", d.dim()))
        .unwrap_or_default();
    println!("  {} {msg}{suffix}", "✓".green().bold());
}

/// Print a warning message.
pub fn warn(msg: &str) {
    println!("  {} {}", "⚠".yellow().bold(), msg.yellow());
}

/// Print an info message.
pub fn info(msg: &str) {
    println!("  {} {}", "ℹ".cyan(), msg.dim());
}

/// Display a tool call above an active spinner.
pub fn tool_call(pb: &ProgressBar, cmd: &str) {
    pb.println(format!("    {} {}", "▸".cyan(), cmd.dim()));
}

/// Format a token count for display (e.g. 1234 -> "1.2k").
pub fn format_tokens(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}k", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

/// Display token usage with optional cost.
pub fn usage(input_tokens: u64, output_tokens: u64, cost_usd: Option<f64>) {
    let cost = cost_usd.map(|c| format!(" · ${c:.4}")).unwrap_or_default();
    println!(
        "  {} {} in / {} out{}",
        "⊘".dim(),
        format_tokens(input_tokens).dim(),
        format_tokens(output_tokens).dim(),
        cost.dim()
    );
}

/// Ask for yes/no confirmation. Returns false in non-TTY environments.
pub fn confirm(prompt: &str) -> Result<bool> {
    if !io::stdin().is_terminal() {
        return Ok(false);
    }

    print!("  {} ", prompt.bold());
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let trimmed = input.trim().to_lowercase();

    Ok(trimmed == "y" || trimmed == "yes")
}

/// Check whether stdout is a terminal (TTY).
pub fn is_tty() -> bool {
    io::stdout().is_terminal()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_tokens_small() {
        assert_eq!(format_tokens(42), "42");
    }

    #[test]
    fn format_tokens_thousands() {
        assert_eq!(format_tokens(1_500), "1.5k");
    }

    #[test]
    fn format_tokens_millions() {
        assert_eq!(format_tokens(2_500_000), "2.5M");
    }
}
