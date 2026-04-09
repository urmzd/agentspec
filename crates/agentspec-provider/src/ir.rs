use serde::{Deserialize, Serialize};

#[cfg(feature = "local")]
use crate::local::LocalBackend;

/// A capability that can be allowed or denied in a sandbox.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Capability {
    /// Read files from the filesystem.
    ReadFile,
    /// Write/edit files.
    WriteFile,
    /// Read-only git operations (log, diff, show, status, blame, etc.).
    GitReadOnly,
    /// Shell commands matching a pattern.
    ShellCommand { pattern: String },
    /// Web/network access.
    Network,
    /// Custom capability (provider-specific escape hatch).
    Custom(String),
}

/// Sandbox configuration — what the AI agent is allowed to do.
///
/// Denied capabilities override allowed ones.
/// An empty/None sandbox means no restrictions.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Sandbox {
    /// Capabilities explicitly allowed.
    pub allowed: Vec<Capability>,
    /// Capabilities explicitly denied (overrides allowed).
    pub denied: Vec<Capability>,
}

/// Provider-agnostic configuration for resolving an AI backend.
#[derive(Debug, Clone, Default)]
pub struct ProviderConfig {
    /// Preferred backend (None = auto-detect).
    #[cfg(feature = "local")]
    pub backend: Option<LocalBackend>,
    /// Model override.
    pub model: Option<String>,
    /// Max budget in USD (providers that support it).
    pub budget: Option<f64>,
    /// Sandbox restrictions.
    pub sandbox: Option<Sandbox>,
    /// Enable debug output.
    pub debug: bool,
}

/// Read-only git subcommands shared across all providers.
pub(crate) const GIT_READONLY_COMMANDS: &[&str] = &[
    "diff", "log", "show", "status", "ls-files", "rev-parse", "branch", "cat-file", "rev-list",
    "shortlog", "blame",
];

/// Internal trait for translating IR sandbox → provider-native arguments.
pub(crate) trait SandboxTranslator {
    /// Convert allowed capabilities into provider-native tool entries.
    fn translate_allowed(&self, capabilities: &[Capability]) -> Vec<String>;

    /// Build the full sandbox arguments from a Sandbox, filtering out denied capabilities.
    fn translate_sandbox(&self, sandbox: &Sandbox) -> Vec<String> {
        let allowed: Vec<_> = sandbox
            .allowed
            .iter()
            .filter(|cap| !sandbox.denied.contains(cap))
            .cloned()
            .collect();
        self.translate_allowed(&allowed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sandbox_default_is_empty() {
        let sandbox = Sandbox::default();
        assert!(sandbox.allowed.is_empty());
        assert!(sandbox.denied.is_empty());
    }

    #[test]
    fn provider_config_default() {
        let config = ProviderConfig::default();
        assert!(config.model.is_none());
        assert!(config.budget.is_none());
        assert!(config.sandbox.is_none());
        assert!(!config.debug);
    }

    #[test]
    fn capability_equality() {
        assert_eq!(Capability::ReadFile, Capability::ReadFile);
        assert_ne!(Capability::ReadFile, Capability::WriteFile);
        assert_eq!(
            Capability::ShellCommand {
                pattern: "ls".into()
            },
            Capability::ShellCommand {
                pattern: "ls".into()
            }
        );
        assert_ne!(
            Capability::ShellCommand {
                pattern: "ls".into()
            },
            Capability::ShellCommand {
                pattern: "cat".into()
            }
        );
    }
}
